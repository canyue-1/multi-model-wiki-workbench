use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use multimodel_wiki_workbench_lib::commands::{
    AddMemberInput, AppState, ProviderFactory, SendMessageInput, SourceInput, SourceInputKind,
};
use multimodel_wiki_workbench_lib::domain::{
    DecisionContext, ModelReply, ProviderKind, ReplyContext, ReviewStatus, SpeakerDecision,
};
use multimodel_wiki_workbench_lib::providers::{ModelProvider, ProviderError};
use multimodel_wiki_workbench_lib::repository::WorkspaceRepository;
use multimodel_wiki_workbench_lib::secrets::MemorySecretStore;
use serde::Deserialize;
use serde_json::Value;
use tempfile::tempdir;

const SENTINEL_KEY: &str = "KEY_SENTINEL_SHOULD_NOT_LEAK_9f1d6d7a";
const SOURCE_EVIDENCE: &str = "统一供应商适配接口";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscussionFixture {
    conversation_title: String,
    message: String,
    members: Vec<MemberFixture>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemberFixture {
    provider: ProviderKind,
    model: String,
    role_name: String,
    role_instruction: String,
    behavior: Behavior,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Behavior {
    Reply,
    Silent,
}

struct FixtureProviderFactory {
    behaviors: HashMap<String, Behavior>,
    observed_contexts: Arc<Mutex<Vec<Value>>>,
}

impl ProviderFactory for FixtureProviderFactory {
    fn build(&self, provider: ProviderKind, api_key: &str, model: &str) -> Arc<dyn ModelProvider> {
        Arc::new(FixtureProvider {
            provider,
            api_key: api_key.to_owned(),
            behavior: self
                .behaviors
                .get(model)
                .copied()
                .unwrap_or(Behavior::Silent),
            observed_contexts: self.observed_contexts.clone(),
        })
    }
}

struct FixtureProvider {
    provider: ProviderKind,
    api_key: String,
    behavior: Behavior,
    observed_contexts: Arc<Mutex<Vec<Value>>>,
}

impl FixtureProvider {
    fn observe<T: serde::Serialize>(&self, context: &T) {
        self.observed_contexts
            .lock()
            .unwrap()
            .push(serde_json::to_value(context).unwrap());
    }
}

#[async_trait]
impl ModelProvider for FixtureProvider {
    fn kind(&self) -> ProviderKind {
        self.provider
    }

    async fn validate_key(&self) -> Result<(), ProviderError> {
        if self.api_key == SENTINEL_KEY {
            Ok(())
        } else {
            Err(ProviderError::InvalidKey)
        }
    }

    async fn decide(&self, context: DecisionContext) -> Result<SpeakerDecision, ProviderError> {
        self.observe(&context);
        match self.behavior {
            Behavior::Reply => Ok(SpeakerDecision::Reply {
                reason: "资料支持形成架构结论".to_owned(),
                priority: 10,
            }),
            Behavior::Silent => Ok(SpeakerDecision::Silent {
                reason: "没有发现需要纠正的矛盾".to_owned(),
            }),
        }
    }

    async fn reply(&self, context: ReplyContext) -> Result<ModelReply, ProviderError> {
        self.observe(&context);
        Ok(ModelReply {
            content: "首版采用统一适配接口，调度器只处理标准化决策。".to_owned(),
            cited_source_ids: Vec::new(),
        })
    }
}

#[tokio::test]
async fn completes_group_chat_to_reviewable_wiki_flow() {
    let fixture = load_fixture();
    let workspace = tempdir().unwrap();
    let database_path = workspace.path().join("data/workbench.sqlite");
    fs::create_dir_all(database_path.parent().unwrap()).unwrap();
    let observed_contexts = Arc::new(Mutex::new(Vec::new()));
    let factory = Arc::new(FixtureProviderFactory {
        behaviors: fixture
            .members
            .iter()
            .map(|member| (member.model.clone(), member.behavior))
            .collect(),
        observed_contexts: observed_contexts.clone(),
    });
    let repository = WorkspaceRepository::open(&database_path).await.unwrap();
    let state = AppState::new(
        workspace.path(),
        repository,
        Arc::new(MemorySecretStore::default()),
        factory,
    );

    for provider in fixture
        .members
        .iter()
        .map(|member| member.provider)
        .collect::<HashSet<_>>()
    {
        state.save_provider_key(provider, SENTINEL_KEY).unwrap();
        state.validate_provider(provider).await.unwrap();
    }

    let conversation = state
        .create_conversation(&fixture.conversation_title)
        .await
        .unwrap();
    for member in fixture.members {
        state
            .add_member(AddMemberInput {
                conversation_id: conversation.id.clone(),
                provider: member.provider,
                model: member.model,
                role_name: member.role_name,
                role_instruction: member.role_instruction,
            })
            .await
            .unwrap();
    }

    let source_path = fixture_path("source.md");
    let source_before = fs::read(&source_path).unwrap();
    let source = state
        .ingest_source(SourceInput {
            conversation_id: Some(conversation.id.clone()),
            kind: SourceInputKind::File,
            value: source_path.to_string_lossy().into_owned(),
        })
        .await
        .unwrap();
    let cycle = state
        .send_message(SendMessageInput {
            conversation_id: conversation.id.clone(),
            content: fixture.message,
            mentioned_member_id: None,
        })
        .await
        .unwrap();

    assert_eq!(cycle.model_message_count, 1);
    let snapshot = state.load_snapshot(&conversation.id).await.unwrap();
    let silent_members: HashSet<_> = snapshot
        .events
        .iter()
        .filter(|event| event.status == "silent")
        .filter_map(|event| event.member_id.as_deref())
        .collect();
    assert_eq!(silent_members.len(), 1);
    assert_eq!(snapshot.sources, vec![source.clone()]);
    assert!(
        observed_contexts
            .lock()
            .unwrap()
            .iter()
            .any(|context| context.to_string().contains(SOURCE_EVIDENCE)),
        "模型上下文应包含已附加资料的可见摘录"
    );

    let reviews = state.list_review_items().await.unwrap();
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0].status, ReviewStatus::Pending);
    assert_eq!(reviews[0].source_ids, vec![source.id.clone()]);
    let pages = state.list_wiki_pages().unwrap();
    assert_eq!(pages.len(), 1);
    assert!(pages[0].path.starts_with("conversations/"));
    assert!(pages[0].markdown.contains("# 多模型路由评审"));
    assert!(pages[0].markdown.contains(SOURCE_EVIDENCE));
    assert!(pages[0].markdown.contains(&source.id));

    state
        .rollback_revision(&reviews[0].revision_id)
        .await
        .unwrap();
    assert!(state.list_wiki_pages().unwrap().is_empty());
    assert_eq!(
        state.list_review_items().await.unwrap()[0].status,
        ReviewStatus::RolledBack
    );
    assert_eq!(fs::read(&source_path).unwrap(), source_before);
    drop(state);

    let reopened_repository = WorkspaceRepository::open(&database_path).await.unwrap();
    let reopened = AppState::new(
        workspace.path(),
        reopened_repository,
        Arc::new(MemorySecretStore::default()),
        Arc::new(FixtureProviderFactory {
            behaviors: HashMap::new(),
            observed_contexts: Arc::new(Mutex::new(Vec::new())),
        }),
    );
    let restored = reopened.load_snapshot(&conversation.id).await.unwrap();
    assert_eq!(restored.thread.messages.len(), 2);
    assert_eq!(restored.sources, vec![source]);
    drop(reopened);

    assert_secret_absent(workspace.path(), SENTINEL_KEY.as_bytes());
}

fn load_fixture() -> DiscussionFixture {
    serde_json::from_slice(&fs::read(fixture_path("two-model-source-discussion.json")).unwrap())
        .unwrap()
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn assert_secret_absent(root: &Path, sentinel: &[u8]) {
    for entry in fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            assert_secret_absent(&path, sentinel);
        } else {
            let bytes = fs::read(&path).unwrap();
            assert!(
                !bytes
                    .windows(sentinel.len())
                    .any(|window| window == sentinel),
                "哨兵密钥泄露到 {}",
                path.display()
            );
        }
    }
}
