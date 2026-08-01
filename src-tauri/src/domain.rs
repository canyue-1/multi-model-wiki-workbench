use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "deepseek")]
    DeepSeek,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::DeepSeek => "deepseek",
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        match value {
            "openai" => Some(Self::OpenAi),
            "anthropic" => Some(Self::Anthropic),
            "gemini" => Some(Self::Gemini),
            "deepseek" => Some(Self::DeepSeek),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelMember {
    pub id: String,
    pub conversation_id: String,
    pub provider: ProviderKind,
    pub model: String,
    pub role_name: String,
    pub role_instruction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub author_kind: String,
    pub author_id: Option<String>,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConversationThread {
    pub conversation: Conversation,
    pub members: Vec<ModelMember>,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SpeakerDecision {
    Reply { reason: String, priority: i32 },
    Silent { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DecisionContext {
    pub conversation_id: String,
    pub trigger_message_id: String,
    pub member: ModelMember,
    pub visible_messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReplyContext {
    pub conversation_id: String,
    pub member: ModelMember,
    pub visible_messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelReply {
    pub content: String,
    pub cited_source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiscussionEvent {
    pub conversation_id: String,
    pub trigger_message_id: String,
    pub mentioned_member_id: Option<String>,
}

impl DiscussionEvent {
    pub fn new(conversation_id: impl Into<String>, trigger_message_id: impl Into<String>) -> Self {
        Self {
            conversation_id: conversation_id.into(),
            trigger_message_id: trigger_message_id.into(),
            mentioned_member_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    AllSilent,
    MessageLimit,
    UserStopped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemberFailure {
    pub member_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CycleState {
    pub model_message_count: usize,
    pub stop_reason: StopReason,
    pub failures: Vec<MemberFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceRecord {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub source_uri: String,
    pub raw_path: String,
    pub content_hash: String,
    pub extracted_text: Option<String>,
    pub extraction_error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReviewStatus {
    Pending,
    Accepted,
    Incorrect,
    RolledBack,
}

impl ReviewStatus {
    pub fn as_storage(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Incorrect => "incorrect",
            Self::RolledBack => "rolled_back",
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "accepted" => Some(Self::Accepted),
            "incorrect" => Some(Self::Incorrect),
            "rolled_back" => Some(Self::RolledBack),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WikiRevision {
    pub id: String,
    pub relative_path: String,
    pub before_content: Option<String>,
    pub after_content: String,
    pub before_hash: Option<String>,
    pub after_hash: String,
    pub source_ids: Vec<String>,
    pub reason: String,
    pub created_at: String,
    pub review_pending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItem {
    pub id: String,
    pub revision_id: String,
    pub path: String,
    pub reason: String,
    pub status: ReviewStatus,
    pub source_ids: Vec<String>,
    pub before_content: Option<String>,
    pub after_content: String,
    pub created_at: String,
    pub reviewed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiscussionRecord {
    pub id: String,
    pub conversation_id: String,
    pub trigger_message_id: Option<String>,
    pub member_id: Option<String>,
    pub kind: String,
    pub status: String,
    pub public_reason: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSnapshot {
    pub thread: ConversationThread,
    pub events: Vec<DiscussionRecord>,
    pub sources: Vec<SourceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WikiPage {
    pub path: String,
    pub title: String,
    pub summary: String,
    pub markdown: String,
}
