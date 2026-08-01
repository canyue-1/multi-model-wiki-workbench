use async_trait::async_trait;
use multimodel_wiki_workbench_lib::domain::{
    DecisionContext, ModelMember, ModelReply, ProviderKind, ReplyContext, SpeakerDecision,
};
use multimodel_wiki_workbench_lib::providers::{
    ModelProvider, ProviderError, parse_speaker_decision,
};
use multimodel_wiki_workbench_lib::secrets::{MemorySecretStore, SecretStore};

struct FakeAdapter {
    decision_json: &'static str,
}

#[async_trait]
impl ModelProvider for FakeAdapter {
    fn kind(&self) -> ProviderKind {
        ProviderKind::OpenAi
    }

    async fn validate_key(&self) -> Result<(), ProviderError> {
        Ok(())
    }

    async fn decide(&self, _context: DecisionContext) -> Result<SpeakerDecision, ProviderError> {
        parse_speaker_decision(self.decision_json)
    }

    async fn reply(&self, _context: ReplyContext) -> Result<ModelReply, ProviderError> {
        Ok(ModelReply {
            content: "补充观点".into(),
            cited_source_ids: Vec::new(),
        })
    }
}

#[tokio::test]
async fn adapters_normalize_speaker_decisions() {
    let adapter = FakeAdapter {
        decision_json: r#"{"decision":"silent","reason":"没有新增信息"}"#,
    };

    let result = adapter.decide(test_context()).await.unwrap();

    assert_eq!(
        result,
        SpeakerDecision::Silent {
            reason: "没有新增信息".into(),
        }
    );
}

#[test]
fn rejects_malformed_or_private_reasoning_decisions() {
    let error = parse_speaker_decision(
        r#"{"decision":"reply","reason":"值得补充","priority":2,"chainOfThought":"secret"}"#,
    )
    .unwrap_err();

    assert!(matches!(error, ProviderError::MalformedDecision(_)));
}

#[test]
fn memory_secret_store_is_scoped_by_provider() {
    let store = MemorySecretStore::default();
    store.save(ProviderKind::OpenAi, "openai-key").unwrap();
    store.save(ProviderKind::Gemini, "gemini-key").unwrap();

    assert_eq!(
        store.load(ProviderKind::OpenAi).unwrap().as_deref(),
        Some("openai-key")
    );
    assert_eq!(
        store.load(ProviderKind::Gemini).unwrap().as_deref(),
        Some("gemini-key")
    );
}

fn test_context() -> DecisionContext {
    DecisionContext {
        conversation_id: "conversation-1".into(),
        trigger_message_id: "message-1".into(),
        member: ModelMember {
            id: "member-1".into(),
            conversation_id: "conversation-1".into(),
            provider: ProviderKind::OpenAi,
            model: "gpt-5".into(),
            role_name: "分析师".into(),
            role_instruction: "分析约束".into(),
        },
        visible_messages: Vec::new(),
        visible_sources: Vec::new(),
    }
}
