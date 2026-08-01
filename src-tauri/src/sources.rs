use std::fs::{self, OpenOptions};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use quick_xml::Reader;
use quick_xml::events::Event;
use scraper::{Html, Selector};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;
use zip::ZipArchive;

use crate::domain::SourceRecord;
use crate::repository::{NewSource, RepositoryError, WorkspaceRepository};

const MAX_WEBPAGE_BYTES: usize = 10 * 1024 * 1024;

pub type IngestedSource = SourceRecord;

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("无法读取资料: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("网页请求失败: {0}")]
    Http(#[from] reqwest::Error),
    #[error("资料路径没有可用文件名")]
    MissingFileName,
    #[error("不支持的资料格式: {0}")]
    UnsupportedFormat(String),
    #[error("不支持的网页地址协议: {0}")]
    UnsupportedUrlScheme(String),
    #[error("网页响应不是 HTML: {0}")]
    UnsupportedMediaType(String),
    #[error("网页快照超过 10 MiB 上限")]
    WebpageTooLarge,
    #[error("原始资料哈希冲突: {0}")]
    HashCollision(String),
}

#[async_trait]
pub trait SourceIngestor {
    async fn ingest_file(&self, path: &Path) -> Result<IngestedSource, IngestError>;
    async fn capture_url(&self, url: &Url) -> Result<IngestedSource, IngestError>;
}

pub struct WorkspaceSourceIngestor {
    workspace_root: PathBuf,
    repository: WorkspaceRepository,
    client: reqwest::Client,
}

impl WorkspaceSourceIngestor {
    pub fn new(
        workspace_root: impl AsRef<Path>,
        repository: WorkspaceRepository,
    ) -> Result<Self, IngestError> {
        Ok(Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
            repository,
            client: reqwest::Client::builder()
                .user_agent("multimodel-wiki-workbench/0.1")
                .build()?,
        })
    }

    async fn store(
        &self,
        kind: &str,
        title: &str,
        source_uri: &str,
        extension: &str,
        bytes: &[u8],
        extraction: Result<String, String>,
    ) -> Result<IngestedSource, IngestError> {
        let content_hash = hex_sha256(bytes);
        let safe_stem = safe_file_stem(title);
        let file_name = format!("{safe_stem}-{}.{}", &content_hash[..12], extension);
        let raw_path = format!("raw/{file_name}");

        if let Some(existing) = self.repository.find_source_by_raw_path(&raw_path).await? {
            if existing.content_hash == content_hash {
                return Ok(existing);
            }
            return Err(IngestError::HashCollision(raw_path));
        }

        let destination = self.workspace_root.join(&raw_path);
        persist_immutable(&destination, bytes, &content_hash)?;
        let (extracted_text, extraction_error) = match extraction {
            Ok(text) => (Some(text), None),
            Err(error) => (None, Some(error)),
        };

        self.repository
            .save_source(NewSource {
                kind,
                title,
                source_uri,
                raw_path: &raw_path,
                content_hash: &content_hash,
                extracted_text: extracted_text.as_deref(),
                extraction_error: extraction_error.as_deref(),
            })
            .await
            .map_err(Into::into)
    }
}

#[async_trait]
impl SourceIngestor for WorkspaceSourceIngestor {
    async fn ingest_file(&self, path: &Path) -> Result<IngestedSource, IngestError> {
        let extension = normalized_extension(path).ok_or(IngestError::MissingFileName)?;
        if !is_supported_extension(&extension) {
            return Err(IngestError::UnsupportedFormat(extension));
        }
        let title = path
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .ok_or(IngestError::MissingFileName)?;
        let bytes = fs::read(path)?;
        let extraction = extract_file(&extension, &bytes);

        self.store(
            "file",
            title,
            &path.to_string_lossy(),
            &extension,
            &bytes,
            extraction,
        )
        .await
    }

    async fn capture_url(&self, url: &Url) -> Result<IngestedSource, IngestError> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(IngestError::UnsupportedUrlScheme(url.scheme().to_owned()));
        }
        let response = self
            .client
            .get(url.clone())
            .send()
            .await?
            .error_for_status()?;
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_owned();
        if !content_type.to_ascii_lowercase().starts_with("text/html") {
            return Err(IngestError::UnsupportedMediaType(content_type));
        }
        let bytes = response.bytes().await?;
        if bytes.len() > MAX_WEBPAGE_BYTES {
            return Err(IngestError::WebpageTooLarge);
        }
        let html = String::from_utf8_lossy(&bytes).into_owned();
        let (title, text) = extract_html(&html, url);

        self.store("url", &title, url.as_str(), "html", &bytes, Ok(text))
            .await
    }
}

fn normalized_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
}

fn is_supported_extension(extension: &str) -> bool {
    matches!(
        extension,
        "txt"
            | "md"
            | "markdown"
            | "docx"
            | "pdf"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "bmp"
            | "tif"
            | "tiff"
    )
}

fn extract_file(extension: &str, bytes: &[u8]) -> Result<String, String> {
    match extension {
        "txt" | "md" | "markdown" => String::from_utf8(bytes.to_vec())
            .map_err(|error| format!("文本不是有效的 UTF-8: {error}")),
        "docx" => extract_docx(bytes),
        "pdf" => pdf_extract::extract_text_from_mem(bytes)
            .map(|text| text.trim().to_owned())
            .map_err(|error| format!("PDF 文本提取失败: {error}")),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" => {
            imagesize::blob_size(bytes)
                .map(|size| {
                    format!(
                        "图片格式: {}\n宽度: {} px\n高度: {} px",
                        extension.to_ascii_uppercase(),
                        size.width,
                        size.height
                    )
                })
                .map_err(|error| format!("图片元数据提取失败: {error}"))
        }
        _ => Err(format!("不支持的资料格式: {extension}")),
    }
}

fn extract_docx(bytes: &[u8]) -> Result<String, String> {
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|error| format!("DOCX 容器无效: {error}"))?;
    let mut document = archive
        .by_name("word/document.xml")
        .map_err(|error| format!("DOCX 缺少正文: {error}"))?;
    let mut xml = String::new();
    document
        .read_to_string(&mut xml)
        .map_err(|error| format!("DOCX 正文读取失败: {error}"))?;

    let mut reader = Reader::from_str(&xml);
    let mut output = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(text)) => {
                let decoded = text
                    .decode()
                    .map_err(|error| format!("DOCX 文字解码失败: {error}"))?;
                output.push_str(&decoded);
            }
            Ok(Event::End(element)) if element.local_name().as_ref() == b"p" => {
                output.push('\n');
            }
            Ok(Event::Empty(element))
                if matches!(element.local_name().as_ref(), b"tab" | b"br" | b"cr") =>
            {
                output.push(if element.local_name().as_ref() == b"tab" {
                    '\t'
                } else {
                    '\n'
                });
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(format!("DOCX XML 解析失败: {error}")),
        }
    }
    let output = output.trim().to_owned();
    if output.is_empty() {
        Err("DOCX 没有可提取的正文".to_owned())
    } else {
        Ok(output)
    }
}

fn extract_html(html: &str, url: &Url) -> (String, String) {
    let document = Html::parse_document(html);
    let title_selector = Selector::parse("title").expect("static title selector is valid");
    let content_selector =
        Selector::parse("h1, h2, h3, h4, h5, h6, p, li, blockquote, pre, td, th, figcaption")
            .expect("static content selector is valid");
    let title = document
        .select(&title_selector)
        .next()
        .map(|element| normalize_whitespace(element.text()))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            url.host_str()
                .filter(|value| !value.is_empty())
                .unwrap_or("网页快照")
                .to_owned()
        });
    let text = document
        .select(&content_selector)
        .map(|element| normalize_whitespace(element.text()))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (title, text)
}

fn normalize_whitespace<'a>(parts: impl Iterator<Item = &'a str>) -> String {
    parts
        .flat_map(str::split_whitespace)
        .collect::<Vec<_>>()
        .join(" ")
}

fn safe_file_stem(title: &str) -> String {
    let mut result = String::with_capacity(title.len());
    for character in title.chars() {
        if character.is_alphanumeric() || matches!(character, '-' | '_') {
            result.push(character);
        } else if !result.ends_with('-') {
            result.push('-');
        }
    }
    let trimmed = result.trim_matches('-');
    if trimmed.is_empty() {
        "source".to_owned()
    } else {
        trimmed.chars().take(80).collect()
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn persist_immutable(path: &Path, bytes: &[u8], expected_hash: &str) -> Result<(), IngestError> {
    let parent = path.parent().ok_or(IngestError::MissingFileName)?;
    fs::create_dir_all(parent)?;
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(bytes)?;
            file.sync_all()?;
            let mut permissions = file.metadata()?.permissions();
            permissions.set_readonly(true);
            fs::set_permissions(path, permissions)?;
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let existing = fs::read(path)?;
            if hex_sha256(&existing) == expected_hash {
                Ok(())
            } else {
                Err(IngestError::HashCollision(path.display().to_string()))
            }
        }
        Err(error) => Err(error.into()),
    }
}
