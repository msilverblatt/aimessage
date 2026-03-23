use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub guid: String,
    pub conversation_id: String,
    pub sender: String,
    pub body: String,
    pub attachments: Vec<String>,
    pub timestamp: DateTime<Utc>,
    pub is_from_me: bool,
    pub status: MessageStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Sent,
    Delivered,
    Read,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub participants: Vec<String>,
    pub display_name: Option<String>,
    pub is_group: bool,
    pub latest_message: Option<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub id: String,
    pub message_id: String,
    pub message_guid: String,
    pub sender: String,
    pub reaction_type: ReactionType,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactionType {
    Love,
    ThumbsUp,
    ThumbsDown,
    HaHa,
    Exclamation,
    Question,
}

impl ReactionType {
    pub fn from_associated_type(t: i64) -> Option<Self> {
        match t {
            2000 | 3000 => Some(ReactionType::Love),
            2001 | 3001 => Some(ReactionType::ThumbsUp),
            2002 | 3002 => Some(ReactionType::ThumbsDown),
            2003 | 3003 => Some(ReactionType::HaHa),
            2004 | 3004 => Some(ReactionType::Exclamation),
            2005 | 3005 => Some(ReactionType::Question),
            _ => None,
        }
    }

    pub fn is_removal(associated_type: i64) -> bool {
        (3000..=3005).contains(&associated_type)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Event {
    #[serde(rename = "message.received")]
    NewMessage(Message),
    #[serde(rename = "message.sent")]
    MessageSent(Message),
    #[serde(rename = "reaction.added")]
    ReactionAdded(Reaction),
    #[serde(rename = "reaction.removed")]
    ReactionRemoved(Reaction),
}

impl Event {
    pub fn event_name(&self) -> &'static str {
        match self {
            Event::NewMessage(_) => "message.received",
            Event::MessageSent(_) => "message.sent",
            Event::ReactionAdded(_) => "reaction.added",
            Event::ReactionRemoved(_) => "reaction.removed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub recipient: String,
    pub body: String,
    #[serde(default)]
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageQuery {
    pub conversation_id: Option<String>,
    pub since: Option<DateTime<Utc>>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationQuery {
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendStatus {
    pub connected: bool,
    pub private_api_available: bool,
    pub message: Option<String>,
}
