use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

use crate::domain::{
    DecisionContext, Message, ModelReply, ProviderKind, ReplyContext, SpeakerDecision,
};

use super::{
    ModelProvider, ProviderError, decision_system, execute_json, extract_text, model_reply,
    openai_messages, parse_speaker_decision, reply_system,
};

pub struct OpenAiCompatibleProvider {
    client: Client,
    api_key: String,
    model: String,
    kind: ProviderKind,
    base_url: &'static str,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        kind: ProviderKind,
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: &'static str,
    ) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            kind,
            base_url,
        }
    }

    async fn complete(
        &self,
        system: String,
        messages: &[Message],
    ) -> Result<String, ProviderError> {
        let value = execute_json(
            self.client
                .post(format!("{}/chat/completions", self.base_url))
                .bearer_auth(&self.api_key)
                .json(&json!({
                    "model": self.model,
                    "messages": openai_messages(system, messages)
                })),
            false,
        )
        .await?;
        Ok(extract_text(&value, &["/choices/0/message/content"])?.to_owned())
    }
}

#[async_trait]
impl ModelProvider for OpenAiCompatibleProvider {
    fn kind(&self) -> ProviderKind {
        self.kind
    }

    async fn validate_key(&self) -> Result<(), ProviderError> {
        execute_json(
            self.client
                .get(format!("{}/models", self.base_url))
                .bearer_auth(&self.api_key),
            true,
        )
        .await
        .map(|_| ())
    }

    async fn decide(&self, context: DecisionContext) -> Result<SpeakerDecision, ProviderError> {
        let text = self
            .complete(
                decision_system(
                    &context.member.role_name,
                    &context.member.role_instruction,
                    &context.visible_sources,
                ),
                &context.visible_messages,
            )
            .await?;
        parse_speaker_decision(&text)
    }

    async fn reply(&self, context: ReplyContext) -> Result<ModelReply, ProviderError> {
        let text = self
            .complete(
                reply_system(
                    &context.member.role_name,
                    &context.member.role_instruction,
                    &context.visible_sources,
                ),
                &context.visible_messages,
            )
            .await?;
        Ok(model_reply(&text))
    }
}
