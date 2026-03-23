use crate::core_layer::types::{Message, MessageStatus, Conversation};

pub fn parse_bb_message(value: &serde_json::Value) -> Option<Message> {
    let guid = value.get("guid")?.as_str()?;
    let text = value.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let is_from_me = value.get("isFromMe").and_then(|v| v.as_bool()).unwrap_or(false);
    let date_created = value.get("dateCreated").and_then(|v| v.as_i64()).unwrap_or(0);

    let chat_guid = value
        .get("chats")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("guid"))
        .and_then(|g| g.as_str())
        .unwrap_or("");

    let handle = value
        .get("handle")
        .and_then(|h| h.get("address"))
        .and_then(|a| a.as_str())
        .unwrap_or("");

    let timestamp = chrono::DateTime::from_timestamp_millis(date_created)
        .unwrap_or_else(chrono::Utc::now);

    Some(Message {
        id: guid.to_string(),
        conversation_id: chat_guid.to_string(),
        sender: if is_from_me { "me".to_string() } else { handle.to_string() },
        body: text.to_string(),
        attachments: vec![],
        timestamp,
        is_from_me,
        status: MessageStatus::Sent,
    })
}

pub fn parse_bb_chat(value: &serde_json::Value) -> Option<Conversation> {
    let guid = value.get("guid")?.as_str()?;
    let display_name = value.get("displayName").and_then(|v| v.as_str()).map(String::from);
    let participants: Vec<String> = value
        .get("participants")
        .and_then(|p| p.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| p.get("address").and_then(|a| a.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let is_group = participants.len() > 1;

    Some(Conversation {
        id: guid.to_string(),
        participants,
        display_name,
        is_group,
        latest_message: None,
    })
}
