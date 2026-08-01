use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use multimodel_wiki_workbench_lib::domain::{
    DecisionContext, DiscussionEvent, ModelReply, ProviderKind, ReplyContext, SpeakerDecision,
    StopReason,
};
use multimodel_wiki_workbench_lib::providers::{ModelProvider, ProviderError};
use multimodel_wiki_workbench_lib::repository::WorkspaceRepository;
use multimodel_wiki_workbench_lib::scheduler::DiscussionScheduler;

#[derive(Clone, Copy)]
enum Behavior {
    Reply,
    Silent,
    Fail,
}

struct FakeProvider {
    behavior: Behavior,
}

#[async_trait]
impl ModelProvider for FakeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::OpenAi
    }

    async fn validate_key(&self) -> Result<(), ProviderError> {
        Ok(())
    }

    async fn decide(&self, _context: DecisionContext) -> Result<SpeakerDecision, ProviderError> {
        match self.behavior {
            Behavior::Reply => Ok(SpeakerDecision::Reply {
                reason: "我有补充".into(),
                priority: 1,
            }),
            Behavior::Silent => Ok(SpeakerDecision::Silent {
                reason: "没有新增信息".into(),
            }),
            Behavior::Fail => Err(ProviderError::Timeout),
        }
    }

    async fn reply(&self, context: ReplyContext) -> Result<ModelReply, ProviderError> {
        Ok(ModelReply {
            content: format!("{} 的回复", context.member.role_name),
            cited_source_ids: Vec::new(),
        })
    }
}

#[tokio::test]
async fn stops_after_twelve_model_messages() {
    let (repo, conversation_id, trigger_id, providers) =
        fixture(&[Behavior::Reply, Behavior::Reply, Behavior::Reply]).await;
    let scheduler = DiscussionScheduler::new(repo.clone(), providers);

    let result = scheduler
        .handle_event(DiscussionEvent::new(conversation_id.clone(), trigger_id))
        .await
        .unwrap();

    assert_eq!(result.model_message_count, 12);
    assert_eq!(result.stop_reason, StopReason::MessageLimit);
    let thread = repo.load_thread(&conversation_id).await.unwrap();
    let model_authors: Vec<&str> = thread
        .messages
        .iter()
        .filter(|message| message.author_kind == "model")
        .filter_map(|message| message.author_id.as_deref())
        .collect();
    assert_eq!(model_authors.len(), 12);
    assert!(model_authors.windows(2).all(|pair| pair[0] != pair[1]));
}

#[tokio::test]
async fn one_provider_failure_does_not_stop_others() {
    let (repo, conversation_id, trigger_id, providers) =
        fixture(&[Behavior::Fail, Behavior::Reply]).await;
    let scheduler = DiscussionScheduler::new(repo, providers);

    let result = scheduler
        .handle_event(DiscussionEvent::new(conversation_id, trigger_id))
        .await
        .unwrap();

    assert_eq!(result.model_message_count, 1);
    assert_eq!(result.failures.len(), 1);
    assert_eq!(result.stop_reason, StopReason::AllSilent);
}

#[tokio::test]
async fn all_silent_models_end_the_cycle() {
    let (repo, conversation_id, trigger_id, providers) =
        fixture(&[Behavior::Silent, Behavior::Silent]).await;
    let scheduler = DiscussionScheduler::new(repo, providers);

    let result = scheduler
        .handle_event(DiscussionEvent::new(conversation_id, trigger_id))
        .await
        .unwrap();

    assert_eq!(result.model_message_count, 0);
    assert_eq!(result.stop_reason, StopReason::AllSilent);
}

async fn fixture(
    behaviors: &[Behavior],
) -> (
    WorkspaceRepository,
    String,
    String,
    HashMap<String, Arc<dyn ModelProvider>>,
) {
    let repo = WorkspaceRepository::in_memory().await.unwrap();
    let conversation = repo.create_conversation("调度测试").await.unwrap();
    let mut providers: HashMap<String, Arc<dyn ModelProvider>> = HashMap::new();

    for (index, behavior) in behaviors.iter().copied().enumerate() {
        let member = repo
            .add_member(
                &conversation.id,
                ProviderKind::OpenAi,
                &format!("model-{index}"),
                &format!("角色-{index}"),
                "提供独立观点",
            )
            .await
            .unwrap();
        providers.insert(member.id, Arc::new(FakeProvider { behavior }));
    }

    let trigger = repo
        .append_message(&conversation.id, "user", None, "开始讨论")
        .await
        .unwrap();
    (repo, conversation.id, trigger.id, providers)
}
