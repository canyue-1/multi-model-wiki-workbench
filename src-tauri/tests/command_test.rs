use std::fs;
use std::sync::Arc;

use async_trait::async_trait;
use multimodel_wiki_workbench_lib::commands::{
    AddMemberInput, AppState, ProviderFactory, SendMessageInput, SourceInput, SourceInputKind,
};
use multimodel_wiki_workbench_lib::domain::{
    DecisionContext, ModelReply, ProviderKind, ReplyContext, SpeakerDecision, StopReason,
};
use multimodel_wiki_workbench_lib::providers::{ModelProvider, ProviderError};
use multimodel_wiki_workbench_lib::repository::WorkspaceRepository;
use multimodel_wiki_workbench_lib::secrets::MemorySecretStore;
use tempfile::tempdir;

struct FakeProviderFactory;

impl ProviderFactory for FakeProviderFactory {
    fn build(&self, provider: ProviderKind, api_key: &str, _model: &str) -> Arc<dyn ModelProvider> {
        Arc::new(FakeProvider {
            provider,
            api_key: api_key.to_owned(),
        })
    }
}

struct FakeProvider {
    provider: ProviderKind,
    api_key: String,
}

#[async_trait]
impl ModelProvider for FakeProvider {
    fn kind(&self) -> ProviderKind {
        self.provider
    }

    async fn validate_key(&self) -> Result<(), ProviderError> {
        if self.api_key == "invalid" {
            Err(ProviderError::InvalidKey)
        } else {
            Ok(())
        }
    }

    async fn decide(&self, _context: DecisionContext) -> Result<SpeakerDecision, ProviderError> {
        Ok(SpeakerDecision::Reply {
            reason: "我能补充一个观点".to_owned(),
            priority: 1,
        })
    }

    async fn reply(&self, context: ReplyContext) -> Result<ModelReply, ProviderError> {
        Ok(ModelReply {
            content: format!("{}：这是模型回复", context.member.role_name),
            cited_source_ids: Vec::new(),
        })
    }
}

#[tokio::test]
async fn maps_invalid_provider_keys_to_a_stable_error_code() {
    let (_workspace, state) = state().await;
    state
        .save_provider_key(ProviderKind::OpenAi, "invalid")
        .unwrap();

    let error = state
        .validate_provider(ProviderKind::OpenAi)
        .await
        .unwrap_err();

    assert_eq!(error.code, "invalid_key");
    assert!(!error.message.contains("invalid"));
}

#[tokio::test]
async fn completes_a_typed_chat_and_source_attachment_flow() {
    let (workspace, state) = state().await;
    state
        .save_provider_key(ProviderKind::OpenAi, "test-key")
        .unwrap();
    state.validate_provider(ProviderKind::OpenAi).await.unwrap();
    let conversation = state.create_conversation("研究讨论").await.unwrap();
    state
        .add_member(AddMemberInput {
            conversation_id: conversation.id.clone(),
            provider: ProviderKind::OpenAi,
            model: "test-model".to_owned(),
            role_name: "分析师".to_owned(),
            role_instruction: "分析关键差异".to_owned(),
        })
        .await
        .unwrap();

    let cycle = state
        .send_message(SendMessageInput {
            conversation_id: conversation.id.clone(),
            content: "开始讨论".to_owned(),
            mentioned_member_id: None,
        })
        .await
        .unwrap();

    assert_eq!(cycle.model_message_count, 1);
    assert_eq!(cycle.stop_reason, StopReason::AllSilent);
    let external = tempdir().unwrap();
    let source_path = external.path().join("note.md");
    fs::write(&source_path, "# 资料\n\n可验证内容").unwrap();
    let source = state
        .ingest_source(SourceInput {
            conversation_id: Some(conversation.id.clone()),
            kind: SourceInputKind::File,
            value: source_path.to_string_lossy().into_owned(),
        })
        .await
        .unwrap();
    assert!(workspace.path().join(&source.raw_path).exists());

    let snapshot = state.load_snapshot(&conversation.id).await.unwrap();
    assert_eq!(snapshot.thread.messages.len(), 2);
    assert_eq!(snapshot.events.len(), 2);
    assert_eq!(snapshot.sources, vec![source]);
}

async fn state() -> (tempfile::TempDir, AppState) {
    let workspace = tempdir().unwrap();
    let repository = WorkspaceRepository::in_memory().await.unwrap();
    let state = AppState::new(
        workspace.path(),
        repository,
        Arc::new(MemorySecretStore::default()),
        Arc::new(FakeProviderFactory),
    );
    (workspace, state)
}
