use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use url::Url;

use crate::domain::{
    Conversation, ConversationSnapshot, CycleState, ModelMember, ProviderKind, ReviewItem,
    ReviewStatus, SourceRecord, WikiPage,
};
use crate::providers::{
    AnthropicProvider, DeepSeekProvider, GeminiProvider, ModelProvider, OpenAiProvider,
    ProviderError,
};
use crate::repository::{RepositoryError, WorkspaceRepository};
use crate::scheduler::{DiscussionScheduler, SchedulerError};
use crate::secrets::{SecretError, SecretStore, SystemSecretStore};
use crate::sources::{IngestError, SourceIngestor, WorkspaceSourceIngestor};
use crate::wiki::{WikiError, WikiService};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
}

impl AppError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddMemberInput {
    pub conversation_id: String,
    pub provider: ProviderKind,
    pub model: String,
    pub role_name: String,
    pub role_instruction: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageInput {
    pub conversation_id: String,
    pub content: String,
    pub mentioned_member_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceInputKind {
    File,
    Url,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInput {
    pub conversation_id: Option<String>,
    pub kind: SourceInputKind,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub provider: ProviderKind,
    pub configured: bool,
}

pub trait ProviderFactory: Send + Sync {
    fn build(&self, provider: ProviderKind, api_key: &str, model: &str) -> Arc<dyn ModelProvider>;
}

pub struct SystemProviderFactory;

impl ProviderFactory for SystemProviderFactory {
    fn build(&self, provider: ProviderKind, api_key: &str, model: &str) -> Arc<dyn ModelProvider> {
        match provider {
            ProviderKind::OpenAi => Arc::new(OpenAiProvider::new(api_key, model)),
            ProviderKind::Anthropic => Arc::new(AnthropicProvider::new(api_key, model)),
            ProviderKind::Gemini => Arc::new(GeminiProvider::new(api_key, model)),
            ProviderKind::DeepSeek => Arc::new(DeepSeekProvider::new(api_key, model)),
        }
    }
}

pub struct AppState {
    workspace_root: PathBuf,
    repository: WorkspaceRepository,
    secret_store: Arc<dyn SecretStore>,
    provider_factory: Arc<dyn ProviderFactory>,
    stop_signals: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl AppState {
    pub fn new(
        workspace_root: impl AsRef<Path>,
        repository: WorkspaceRepository,
        secret_store: Arc<dyn SecretStore>,
        provider_factory: Arc<dyn ProviderFactory>,
    ) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
            repository,
            secret_store,
            provider_factory,
            stop_signals: Mutex::new(HashMap::new()),
        }
    }

    pub fn save_provider_key(&self, provider: ProviderKind, api_key: &str) -> Result<(), AppError> {
        if api_key.trim().is_empty() {
            return Err(AppError::new("empty_key", "API Key 不能为空"));
        }
        self.secret_store.save(provider, api_key)?;
        Ok(())
    }

    pub fn delete_provider_key(&self, provider: ProviderKind) -> Result<(), AppError> {
        self.secret_store.delete(provider)?;
        Ok(())
    }

    pub async fn validate_provider(&self, provider: ProviderKind) -> Result<(), AppError> {
        let api_key = self.require_key(provider)?;
        self.provider_factory
            .build(provider, &api_key, default_model(provider))
            .validate_key()
            .await?;
        Ok(())
    }

    pub fn provider_statuses(&self) -> Result<Vec<ProviderStatus>, AppError> {
        ProviderKind::ALL
            .into_iter()
            .map(|provider| {
                Ok(ProviderStatus {
                    provider,
                    configured: self.secret_store.load(provider)?.is_some(),
                })
            })
            .collect()
    }

    pub async fn create_conversation(&self, title: &str) -> Result<Conversation, AppError> {
        let title = required_text(title, "会话标题不能为空")?;
        Ok(self.repository.create_conversation(title).await?)
    }

    pub async fn list_conversations(&self) -> Result<Vec<Conversation>, AppError> {
        Ok(self.repository.list_conversations().await?)
    }

    pub async fn add_member(&self, input: AddMemberInput) -> Result<ModelMember, AppError> {
        let model = required_text(&input.model, "模型名称不能为空")?;
        let role_name = required_text(&input.role_name, "角色名称不能为空")?;
        let role_instruction = required_text(&input.role_instruction, "角色指令不能为空")?;
        Ok(self
            .repository
            .add_member(
                &input.conversation_id,
                input.provider,
                model,
                role_name,
                role_instruction,
            )
            .await?)
    }

    pub async fn load_snapshot(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationSnapshot, AppError> {
        Ok(ConversationSnapshot {
            thread: self.repository.load_thread(conversation_id).await?,
            events: self.repository.load_events(conversation_id).await?,
            sources: self.repository.list_sources(Some(conversation_id)).await?,
        })
    }

    pub async fn send_message(&self, input: SendMessageInput) -> Result<CycleState, AppError> {
        let content = required_text(&input.content, "消息不能为空")?;
        let thread = self.repository.load_thread(&input.conversation_id).await?;
        let message = self
            .repository
            .append_message(&input.conversation_id, "user", None, content)
            .await?;
        let mut providers = HashMap::new();
        for member in thread.members {
            if let Some(api_key) = self.secret_store.load(member.provider)? {
                providers.insert(
                    member.id,
                    self.provider_factory
                        .build(member.provider, &api_key, &member.model),
                );
            }
        }
        let stop_signal = self.stop_signal(&input.conversation_id)?;
        stop_signal.store(false, Ordering::Relaxed);
        let scheduler = DiscussionScheduler::new(self.repository.clone(), providers)
            .with_stop_signal(stop_signal);
        let mut event = crate::domain::DiscussionEvent::new(input.conversation_id, message.id);
        event.mentioned_member_id = input.mentioned_member_id;
        Ok(scheduler.handle_event(event).await?)
    }

    pub fn stop_discussion(&self, conversation_id: &str) -> Result<(), AppError> {
        self.stop_signal(conversation_id)?
            .store(true, Ordering::Relaxed);
        Ok(())
    }

    pub async fn ingest_source(&self, input: SourceInput) -> Result<SourceRecord, AppError> {
        let ingestor = WorkspaceSourceIngestor::new(&self.workspace_root, self.repository.clone())?;
        let source = match input.kind {
            SourceInputKind::File => ingestor.ingest_file(Path::new(&input.value)).await?,
            SourceInputKind::Url => {
                let url = Url::parse(&input.value)
                    .map_err(|_| AppError::new("invalid_url", "网页地址无效"))?;
                ingestor.capture_url(&url).await?
            }
        };
        if let Some(conversation_id) = input.conversation_id {
            self.repository
                .attach_source(&conversation_id, &source.id)
                .await?;
        }
        Ok(source)
    }

    pub async fn list_review_items(&self) -> Result<Vec<ReviewItem>, AppError> {
        Ok(self.repository.list_review_items().await?)
    }

    pub fn list_wiki_pages(&self) -> Result<Vec<WikiPage>, AppError> {
        Ok(WikiService::new(&self.workspace_root, self.repository.clone()).list_pages()?)
    }

    pub async fn set_review_status(
        &self,
        revision_id: &str,
        status: ReviewStatus,
    ) -> Result<(), AppError> {
        WikiService::new(&self.workspace_root, self.repository.clone())
            .set_review_status(revision_id, status)
            .await?;
        Ok(())
    }

    pub async fn rollback_revision(&self, revision_id: &str) -> Result<(), AppError> {
        WikiService::new(&self.workspace_root, self.repository.clone())
            .rollback(revision_id)
            .await?;
        Ok(())
    }

    fn require_key(&self, provider: ProviderKind) -> Result<String, AppError> {
        self.secret_store
            .load(provider)?
            .ok_or_else(|| AppError::new("missing_key", "请先保存该供应商的 API Key"))
    }

    fn stop_signal(&self, conversation_id: &str) -> Result<Arc<AtomicBool>, AppError> {
        let mut signals = self
            .stop_signals
            .lock()
            .map_err(|_| AppError::new("state_unavailable", "讨论状态暂时不可用"))?;
        Ok(signals
            .entry(conversation_id.to_owned())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone())
    }
}

impl ProviderKind {
    pub const ALL: [Self; 4] = [Self::OpenAi, Self::Anthropic, Self::Gemini, Self::DeepSeek];
}

fn default_model(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::OpenAi => "gpt-5-mini",
        ProviderKind::Anthropic => "claude-sonnet-4-5",
        ProviderKind::Gemini => "gemini-2.5-flash",
        ProviderKind::DeepSeek => "deepseek-chat",
    }
}

fn required_text<'a>(value: &'a str, message: &str) -> Result<&'a str, AppError> {
    let value = value.trim();
    if value.is_empty() {
        Err(AppError::new("invalid_input", message))
    } else {
        Ok(value)
    }
}

impl From<ProviderError> for AppError {
    fn from(error: ProviderError) -> Self {
        match error {
            ProviderError::InvalidKey => Self::new("invalid_key", "API Key 无效或没有访问权限"),
            ProviderError::Quota => Self::new("quota_exhausted", "供应商额度不足或请求过于频繁"),
            ProviderError::Timeout => Self::new("provider_timeout", "供应商请求超时"),
            ProviderError::Transport(_) => Self::new("provider_unreachable", "无法连接到供应商"),
            ProviderError::MalformedDecision(_) => {
                Self::new("malformed_decision", "模型返回的发言决策格式无效")
            }
            ProviderError::Remote(_) => Self::new("provider_error", "供应商返回了异常响应"),
        }
    }
}

impl From<RepositoryError> for AppError {
    fn from(_: RepositoryError) -> Self {
        Self::new("database_error", "本地数据库操作失败")
    }
}

impl From<SecretError> for AppError {
    fn from(_: SecretError) -> Self {
        Self::new("credential_store_error", "系统凭据库操作失败")
    }
}

impl From<SchedulerError> for AppError {
    fn from(error: SchedulerError) -> Self {
        match error {
            SchedulerError::Repository(error) => error.into(),
        }
    }
}

impl From<IngestError> for AppError {
    fn from(error: IngestError) -> Self {
        match error {
            IngestError::UnsupportedFormat(extension) => {
                Self::new("unsupported_format", format!("暂不支持 .{extension} 格式"))
            }
            IngestError::UnsupportedUrlScheme(_) => {
                Self::new("invalid_url", "仅支持 HTTP 或 HTTPS 网页地址")
            }
            IngestError::UnsupportedMediaType(_) => {
                Self::new("unsupported_webpage", "该地址不是普通 HTML 网页")
            }
            IngestError::WebpageTooLarge => {
                Self::new("webpage_too_large", "网页快照超过 10 MiB 上限")
            }
            IngestError::Http(_) => Self::new("source_network_error", "网页抓取失败"),
            IngestError::MissingFileName | IngestError::Io(_) => {
                Self::new("source_io_error", "无法读取所选资料")
            }
            IngestError::HashCollision(_) => {
                Self::new("source_conflict", "原始资料存储发生哈希冲突")
            }
            IngestError::Repository(error) => error.into(),
        }
    }
}

impl From<WikiError> for AppError {
    fn from(error: WikiError) -> Self {
        match error {
            WikiError::InvalidPath(_) => Self::new("invalid_wiki_path", "Wiki 路径无效"),
            WikiError::StaleRevision => {
                Self::new("stale_revision", "该修订已被后续内容取代，无法回退")
            }
            WikiError::Repository(error) => error.into(),
            WikiError::Io(_) | WikiError::TimeFormat(_) => {
                Self::new("wiki_io_error", "Wiki 文件操作失败")
            }
        }
    }
}

pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let workspace_root = app.path().app_data_dir()?;
    std::fs::create_dir_all(workspace_root.join("data"))?;
    let repository = tauri::async_runtime::block_on(WorkspaceRepository::open(
        &workspace_root.join("data/workbench.sqlite"),
    ))?;
    app.manage(AppState::new(
        workspace_root,
        repository,
        Arc::new(SystemSecretStore),
        Arc::new(SystemProviderFactory),
    ));
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub fn save_provider_key(
    state: State<'_, AppState>,
    provider: ProviderKind,
    api_key: String,
) -> Result<(), AppError> {
    state.save_provider_key(provider, &api_key)
}

#[tauri::command(rename_all = "camelCase")]
pub fn delete_provider_key(
    state: State<'_, AppState>,
    provider: ProviderKind,
) -> Result<(), AppError> {
    state.delete_provider_key(provider)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn validate_provider(
    state: State<'_, AppState>,
    provider: ProviderKind,
) -> Result<(), AppError> {
    state.validate_provider(provider).await
}

#[tauri::command]
pub fn provider_statuses(state: State<'_, AppState>) -> Result<Vec<ProviderStatus>, AppError> {
    state.provider_statuses()
}

#[tauri::command]
pub async fn create_conversation(
    state: State<'_, AppState>,
    title: String,
) -> Result<Conversation, AppError> {
    state.create_conversation(&title).await
}

#[tauri::command]
pub async fn list_conversations(state: State<'_, AppState>) -> Result<Vec<Conversation>, AppError> {
    state.list_conversations().await
}

#[tauri::command]
pub async fn add_member(
    state: State<'_, AppState>,
    input: AddMemberInput,
) -> Result<ModelMember, AppError> {
    state.add_member(input).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn load_snapshot(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<ConversationSnapshot, AppError> {
    state.load_snapshot(&conversation_id).await
}

#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    input: SendMessageInput,
) -> Result<CycleState, AppError> {
    state.send_message(input).await
}

#[tauri::command(rename_all = "camelCase")]
pub fn stop_discussion(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<(), AppError> {
    state.stop_discussion(&conversation_id)
}

#[tauri::command]
pub async fn ingest_source(
    state: State<'_, AppState>,
    input: SourceInput,
) -> Result<SourceRecord, AppError> {
    state.ingest_source(input).await
}

#[tauri::command]
pub async fn list_review_items(state: State<'_, AppState>) -> Result<Vec<ReviewItem>, AppError> {
    state.list_review_items().await
}

#[tauri::command]
pub fn list_wiki_pages(state: State<'_, AppState>) -> Result<Vec<WikiPage>, AppError> {
    state.list_wiki_pages()
}

#[tauri::command(rename_all = "camelCase")]
pub async fn set_review_status(
    state: State<'_, AppState>,
    revision_id: String,
    status: ReviewStatus,
) -> Result<(), AppError> {
    state.set_review_status(&revision_id, status).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn rollback_revision(
    state: State<'_, AppState>,
    revision_id: String,
) -> Result<(), AppError> {
    state.rollback_revision(&revision_id).await
}
