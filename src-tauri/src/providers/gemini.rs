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

pub struct GeminiProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl GeminiProvider {
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
        let contents: Vec<Value> = messages
            .iter()
            .map(|message| {
                let role = if message.author_kind == "model" {
                    "model"
                } else {
                    "user"
                };
                json!({ "role": role, "parts": [{ "text": message.content }] })
            })
            .collect();
        let value = execute_json(
            self.client
                .post(format!(
                    "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
                    self.model
                ))
                .header("x-goog-api-key", &self.api_key)
                .json(&json!({
                    "systemInstruction": { "parts": [{ "text": system }] },
                    "contents": contents
                })),
            false,
        )
        .await?;
        Ok(extract_text(&value, &["/candidates/0/content/parts/0/text"])?.to_owned())
    }
}

#[async_trait]
impl ModelProvider for GeminiProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Gemini
    }

    async fn validate_key(&self) -> Result<(), ProviderError> {
        execute_json(
            self.client
                .get("https://generativelanguage.googleapis.com/v1beta/models")
                .header("x-goog-api-key", &self.api_key),
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
