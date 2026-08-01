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

pub struct DeepSeekProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl DeepSeekProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    async fn complete(
        &self,
        system: String,
        messages: &[Message],
    ) -> Result<String, ProviderError> {
        let value = execute_json(
            self.client
                .post("https://api.deepseek.com/chat/completions")
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
impl ModelProvider for DeepSeekProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::DeepSeek
    }

    async fn validate_key(&self) -> Result<(), ProviderError> {
        execute_json(
            self.client
                .get("https://api.deepseek.com/models")
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
