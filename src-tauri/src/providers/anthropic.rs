use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Value, json};

use crate::domain::{
    DecisionContext, Message, ModelReply, ProviderKind, ReplyContext, SpeakerDecision,
};

use super::{
    ModelProvider, ProviderError, decision_system, execute_json, extract_text, model_reply,
    parse_speaker_decision, reply_system,
};

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl AnthropicProvider {
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
        let messages: Vec<Value> = messages
            .iter()
            .map(|message| {
                let role = if message.author_kind == "model" {
                    "assistant"
                } else {
                    "user"
                };
                json!({ "role": role, "content": message.content })
            })
            .collect();
        let value = execute_json(
            self.client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&json!({
                    "model": self.model,
                    "max_tokens": 2048,
                    "system": system,
                    "messages": messages
                })),
            false,
        )
        .await?;
        Ok(extract_text(&value, &["/content/0/text"])?.to_owned())
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Anthropic
    }

    async fn validate_key(&self) -> Result<(), ProviderError> {
        execute_json(
            self.client
                .get("https://api.anthropic.com/v1/models")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01"),
            true,
        )
        .await
        .map(|_| ())
    }

    async fn decide(&self, context: DecisionContext) -> Result<SpeakerDecision, ProviderError> {
        let text = self
            .complete(
                decision_system(&context.member.role_name, &context.member.role_instruction),
                &context.visible_messages,
            )
            .await?;
        parse_speaker_decision(&text)
    }

    async fn reply(&self, context: ReplyContext) -> Result<ModelReply, ProviderError> {
        let text = self
            .complete(
                reply_system(&context.member.role_name, &context.member.role_instruction),
                &context.visible_messages,
            )
            .await?;
        Ok(model_reply(&text))
    }
}
