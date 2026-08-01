use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use atomic_write_file::AtomicWriteFile;
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::domain::{ReviewStatus, WikiPage, WikiRevision};
use crate::repository::{NewWikiRevision, RepositoryError, WorkspaceRepository};

#[derive(Debug, Clone)]
pub struct WikiChange {
    pub relative_path: PathBuf,
    pub markdown: String,
    pub source_ids: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Error)]
pub enum WikiError {
    #[error("Wiki 文件操作失败: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("无效的 Wiki 路径: {0}")]
    InvalidPath(String),
    #[error("该修订已被后续内容取代，不能直接回退")]
    StaleRevision,
    #[error("无法格式化 Wiki 日志时间: {0}")]
    TimeFormat(#[from] time::error::Format),
}

pub struct WikiService {
    workspace_root: PathBuf,
    repository: WorkspaceRepository,
}

impl WikiService {
    pub fn new(workspace_root: impl AsRef<Path>, repository: WorkspaceRepository) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
            repository,
        }
    }

    pub async fn apply(&self, change: WikiChange) -> Result<WikiRevision, WikiError> {
        let relative_path = validate_relative_path(&change.relative_path)?;
        let destination = self.prepare_destination(&relative_path)?;
        let before_content = if destination.exists() {
            Some(fs::read_to_string(&destination)?)
        } else {
            None
        };
        let before_hash = before_content.as_deref().map(hex_sha256);
        let after_hash = hex_sha256(&change.markdown);
        atomic_write(&destination, &change.markdown)?;

        let revision = self
            .repository
            .create_wiki_revision(NewWikiRevision {
                relative_path: &path_for_storage(&relative_path)?,
                before_content: before_content.as_deref(),
                after_content: &change.markdown,
                before_hash: before_hash.as_deref(),
                after_hash: &after_hash,
                source_ids: &change.source_ids,
                reason: &change.reason,
            })
            .await?;
        self.rebuild_index()?;
        self.append_log(
            "apply",
            &revision.relative_path,
            &change.reason,
            &change.source_ids,
        )?;
        Ok(revision)
    }

    pub async fn rollback(&self, revision_id: &str) -> Result<(), WikiError> {
        let revision = self.repository.load_wiki_revision(revision_id).await?;
        let relative_path = validate_relative_path(Path::new(&revision.relative_path))?;
        let destination = self.prepare_destination(&relative_path)?;
        let current = if destination.exists() {
            Some(fs::read_to_string(&destination)?)
        } else {
            None
        };
        if current.as_deref().map(hex_sha256).as_deref() != Some(&revision.after_hash) {
            return Err(WikiError::StaleRevision);
        }

        if let Some(before) = &revision.before_content {
            atomic_write(&destination, before)?;
        } else {
            fs::remove_file(&destination)?;
        }
        self.repository
            .set_review_status(revision_id, ReviewStatus::RolledBack)
            .await?;
        self.rebuild_index()?;
        self.append_log(
            "rollback",
            &revision.relative_path,
            &revision.reason,
            &revision.source_ids,
        )?;
        Ok(())
    }

    pub async fn set_review_status(
        &self,
        revision_id: &str,
        status: ReviewStatus,
    ) -> Result<(), WikiError> {
        if matches!(status, ReviewStatus::Pending | ReviewStatus::RolledBack) {
            return Err(WikiError::InvalidPath(
                "复核操作仅允许接受或标记错误".to_owned(),
            ));
        }
        self.repository
            .set_review_status(revision_id, status)
            .await?;
        Ok(())
    }

    pub fn list_pages(&self) -> Result<Vec<WikiPage>, WikiError> {
        let root = self.ensure_wiki_root()?;
        let mut pages = Vec::new();
        collect_pages(&root, &root, &mut pages)?;
        pages.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(pages)
    }

    fn wiki_root(&self) -> PathBuf {
        self.workspace_root.join("wiki")
    }

    fn ensure_wiki_root(&self) -> Result<PathBuf, WikiError> {
        let root = self.wiki_root();
        if root.exists() && fs::symlink_metadata(&root)?.file_type().is_symlink() {
            return Err(WikiError::InvalidPath("wiki 目录不能是符号链接".to_owned()));
        }
        fs::create_dir_all(&root)?;
        Ok(root)
    }

    fn prepare_destination(&self, relative_path: &Path) -> Result<PathBuf, WikiError> {
        let root = self.ensure_wiki_root()?;
        let parent = relative_path.parent().unwrap_or_else(|| Path::new(""));
        let mut current = root.clone();
        for component in parent.components() {
            let Component::Normal(segment) = component else {
                return Err(WikiError::InvalidPath(relative_path.display().to_string()));
            };
            current.push(segment);
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(WikiError::InvalidPath(format!(
                        "父目录不能是符号链接: {}",
                        current.display()
                    )));
                }
                Ok(metadata) if !metadata.is_dir() => {
                    return Err(WikiError::InvalidPath(format!(
                        "父路径不是目录: {}",
                        current.display()
                    )));
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    fs::create_dir(&current)?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        let destination = root.join(relative_path);
        if destination.exists() && fs::symlink_metadata(&destination)?.file_type().is_symlink() {
            return Err(WikiError::InvalidPath(format!(
                "Wiki 页面不能是符号链接: {}",
                relative_path.display()
            )));
        }
        Ok(destination)
    }

    fn rebuild_index(&self) -> Result<(), WikiError> {
        let root = self.ensure_wiki_root()?;
        let pages = self.list_pages()?;
        let mut index = String::from("# Wiki Index\n\n");
        if pages.is_empty() {
            index.push_str("暂无页面。\n");
        } else {
            for page in pages {
                index.push_str(&format!("- [{}]({})", page.title, page.path));
                if !page.summary.is_empty() {
                    index.push_str(&format!(" - {}", page.summary));
                }
                index.push('\n');
            }
        }
        atomic_write(&root.join("index.md"), &index)?;
        Ok(())
    }

    fn append_log(
        &self,
        operation: &str,
        relative_path: &str,
        reason: &str,
        source_ids: &[String],
    ) -> Result<(), WikiError> {
        let root = self.ensure_wiki_root()?;
        let path = root.join("log.md");
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(WikiError::InvalidPath("log.md 不能是符号链接".to_owned()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        if file.metadata()?.len() == 0 {
            file.write_all(b"# Wiki Log\n\n")?;
        }
        let timestamp = OffsetDateTime::now_utc().format(&Rfc3339)?;
        writeln!(
            file,
            "## [{timestamp}] {} | {}",
            one_line(operation),
            one_line(relative_path)
        )?;
        writeln!(file, "- Reason: {}", one_line(reason))?;
        writeln!(
            file,
            "- Sources: {}\n",
            source_ids
                .iter()
                .map(|source| one_line(source))
                .collect::<Vec<_>>()
                .join(", ")
        )?;
        file.sync_all()?;
        Ok(())
    }
}

fn validate_relative_path(path: &Path) -> Result<PathBuf, WikiError> {
    let markdown_extension = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("md"));
    if path.as_os_str().is_empty() || !markdown_extension {
        return Err(WikiError::InvalidPath(path.display().to_string()));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            _ => return Err(WikiError::InvalidPath(path.display().to_string())),
        }
    }
    let stored_path = path_for_storage(&normalized)?;
    if stored_path.eq_ignore_ascii_case("index.md") || stored_path.eq_ignore_ascii_case("log.md") {
        return Err(WikiError::InvalidPath(
            "index.md 和 log.md 由系统维护".to_owned(),
        ));
    }
    Ok(normalized)
}

fn collect_pages(
    root: &Path,
    directory: &Path,
    pages: &mut Vec<WikiPage>,
) -> Result<(), WikiError> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let metadata = entry.file_type()?;
        if metadata.is_symlink() {
            continue;
        }
        let path = entry.path();
        if metadata.is_dir() {
            collect_pages(root, &path, pages)?;
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let relative = path.strip_prefix(root).map_err(|_| {
            WikiError::InvalidPath(format!("页面不在 Wiki 目录内: {}", path.display()))
        })?;
        let relative = path_for_storage(relative)?;
        if matches!(relative.as_str(), "index.md" | "log.md") {
            continue;
        }
        let content = fs::read_to_string(path)?;
        let (title, summary) = page_metadata(&content, &relative);
        pages.push(WikiPage {
            path: relative,
            title,
            summary,
            markdown: content,
        });
    }
    Ok(())
}

fn page_metadata(content: &str, relative_path: &str) -> (String, String) {
    let title = content
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            Path::new(relative_path)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or(relative_path)
                .to_owned()
        });
    let summary = content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#') && *line != "---")
        .unwrap_or("")
        .chars()
        .take(160)
        .collect();
    (title, summary)
}

fn atomic_write(path: &Path, content: &str) -> Result<(), io::Error> {
    let mut file = AtomicWriteFile::options().open(path)?;
    file.write_all(content.as_bytes())?;
    file.commit()
}

fn path_for_storage(path: &Path) -> Result<String, WikiError> {
    path.to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or_else(|| WikiError::InvalidPath(path.display().to_string()))
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn hex_sha256(content: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(content.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}
