use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::path::Path;

use crate::core_layer::types::*;

/// Mac epoch offset: seconds between Unix epoch (1970) and Mac epoch (2001)
const MAC_EPOCH_OFFSET: i64 = 978_307_200;
/// Threshold for detecting nanosecond timestamps
const NANO_THRESHOLD: i64 = 1_000_000_000_000;

pub struct ChatDb {
    conn: Connection,
}

impl ChatDb {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("Failed to open chat.db: {}", e))?;
        Ok(ChatDb { conn })
    }

    /// Convert a Mac absolute time to UTC DateTime
    fn mac_to_utc(mac_time: i64) -> chrono::DateTime<chrono::Utc> {
        let seconds = if mac_time > NANO_THRESHOLD {
            mac_time / 1_000_000_000
        } else {
            mac_time
        };
        let unix_ts = seconds + MAC_EPOCH_OFFSET;
        chrono::DateTime::from_timestamp(unix_ts, 0).unwrap_or_else(chrono::Utc::now)
    }

    /// Get all new messages and reactions since the given ROWID
    pub fn poll_new_events(&self, since_rowid: i64) -> Result<(Vec<Event>, i64), String> {
        let mut events = Vec::new();
        let mut max_rowid = since_rowid;

        let mut stmt = self.conn.prepare(
            "SELECT m.ROWID, m.guid, m.text, m.handle_id, m.date, m.is_from_me,
                    m.associated_message_guid, m.associated_message_type,
                    m.cache_has_attachments,
                    h.id as handle_address,
                    c.guid as chat_guid, c.display_name, c.chat_identifier
             FROM message m
             LEFT JOIN handle h ON m.handle_id = h.ROWID
             LEFT JOIN chat_message_join cmj ON m.ROWID = cmj.message_id
             LEFT JOIN chat c ON cmj.chat_id = c.ROWID
             WHERE m.ROWID > ?1
             ORDER BY m.ROWID ASC"
        ).map_err(|e| format!("Failed to prepare poll query: {}", e))?;

        let rows = stmt.query_map(params![since_rowid], |row| {
            let rowid: i64 = row.get(0)?;
            let guid: String = row.get(1)?;
            let text: Option<String> = row.get(2)?;
            let _handle_id: Option<i64> = row.get(3)?;
            let date: i64 = row.get(4)?;
            let is_from_me: bool = row.get(5)?;
            let assoc_guid: Option<String> = row.get(6)?;
            let assoc_type: i64 = row.get::<_, Option<i64>>(7)?.unwrap_or(0);
            let _has_attachments: bool = row.get::<_, Option<bool>>(8)?.unwrap_or(false);
            let handle_address: Option<String> = row.get(9)?;
            let chat_guid: Option<String> = row.get(10)?;
            let _display_name: Option<String> = row.get(11)?;
            let _chat_identifier: Option<String> = row.get(12)?;

            Ok((rowid, guid, text, date, is_from_me, assoc_guid, assoc_type,
                handle_address, chat_guid))
        }).map_err(|e| format!("Failed to poll: {}", e))?;

        for row in rows {
            let (rowid, guid, text, date, is_from_me, assoc_guid, assoc_type,
                 handle_address, chat_guid) = row.map_err(|e| format!("Row error: {}", e))?;

            if rowid > max_rowid {
                max_rowid = rowid;
            }

            let timestamp = Self::mac_to_utc(date);
            let sender = if is_from_me {
                "me".to_string()
            } else {
                handle_address.unwrap_or_default()
            };
            let conv_id = chat_guid.unwrap_or_default();

            if assoc_type >= 2000 && assoc_type <= 3005 {
                // This is a reaction
                if let Some(reaction_type) = ReactionType::from_associated_type(assoc_type) {
                    let target_guid = assoc_guid.unwrap_or_default();
                    // Look up the target message's ROWID by its guid
                    let target_rowid = self.rowid_for_guid(&target_guid)
                        .unwrap_or_default()
                        .map(|r| r.to_string())
                        .unwrap_or_default();

                    let reaction = Reaction {
                        id: rowid.to_string(),
                        message_id: target_rowid,
                        message_guid: target_guid,
                        sender,
                        reaction_type,
                        timestamp,
                    };

                    if ReactionType::is_removal(assoc_type) {
                        events.push(Event::ReactionRemoved(reaction));
                    } else {
                        events.push(Event::ReactionAdded(reaction));
                    }
                }
            } else {
                // Regular message
                let message = Message {
                    id: rowid.to_string(),
                    guid,
                    conversation_id: conv_id,
                    sender,
                    body: text.unwrap_or_default(),
                    attachments: vec![], // TODO: attachment support post-MVP
                    timestamp,
                    is_from_me,
                    status: if is_from_me { MessageStatus::Sent } else { MessageStatus::Delivered },
                };

                if is_from_me {
                    events.push(Event::MessageSent(message));
                } else {
                    events.push(Event::NewMessage(message));
                }
            }
        }

        Ok((events, max_rowid))
    }

    /// Look up a message ROWID by its guid
    fn rowid_for_guid(&self, guid: &str) -> Result<Option<i64>, String> {
        // associated_message_guid has format "p:0/<guid>" — extract the guid part
        let clean_guid = if let Some(pos) = guid.rfind('/') {
            &guid[pos + 1..]
        } else {
            guid
        };

        self.conn.query_row(
            "SELECT ROWID FROM message WHERE guid = ?1",
            params![clean_guid],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Failed to look up guid: {}", e))
    }

    /// Get messages with query parameters (for GET /api/v1/messages)
    pub fn get_messages(&self, query: &MessageQuery) -> Result<Vec<Message>, String> {
        let mut sql = String::from(
            "SELECT m.ROWID, m.guid, m.text, m.date, m.is_from_me,
                    h.id as handle_address, c.guid as chat_guid
             FROM message m
             LEFT JOIN handle h ON m.handle_id = h.ROWID
             LEFT JOIN chat_message_join cmj ON m.ROWID = cmj.message_id
             LEFT JOIN chat c ON cmj.chat_id = c.ROWID
             WHERE m.associated_message_type = 0"
        );

        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1;

        if let Some(ref conv_id) = query.conversation_id {
            sql.push_str(&format!(" AND c.guid = ?{}", param_idx));
            param_values.push(Box::new(conv_id.clone()));
            param_idx += 1;
        }

        if let Some(since) = query.since {
            let mac_time = since.timestamp() - MAC_EPOCH_OFFSET;
            sql.push_str(&format!(" AND m.date > ?{}", param_idx));
            param_values.push(Box::new(mac_time));
            param_idx += 1;
        }

        sql.push_str(" ORDER BY m.ROWID DESC");
        sql.push_str(&format!(" LIMIT ?{}", param_idx));
        param_values.push(Box::new(query.limit));
        param_idx += 1;
        sql.push_str(&format!(" OFFSET ?{}", param_idx));
        param_values.push(Box::new(query.offset));

        let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt.query_map(params_ref.as_slice(), |row| {
            let rowid: i64 = row.get(0)?;
            let guid: String = row.get(1)?;
            let text: Option<String> = row.get(2)?;
            let date: i64 = row.get(3)?;
            let is_from_me: bool = row.get(4)?;
            let handle: Option<String> = row.get(5)?;
            let chat_guid: Option<String> = row.get(6)?;

            Ok(Message {
                id: rowid.to_string(),
                guid,
                conversation_id: chat_guid.unwrap_or_default(),
                sender: if is_from_me { "me".to_string() } else { handle.unwrap_or_default() },
                body: text.unwrap_or_default(),
                attachments: vec![],
                timestamp: Self::mac_to_utc(date),
                is_from_me,
                status: if is_from_me { MessageStatus::Sent } else { MessageStatus::Delivered },
            })
        }).map_err(|e| format!("Failed to query messages: {}", e))?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(messages)
    }

    /// Get a single message by ROWID
    pub fn get_message(&self, rowid: &str) -> Result<Message, String> {
        let rowid_int: i64 = rowid.parse().map_err(|_| format!("Invalid message id: {}", rowid))?;

        self.conn.query_row(
            "SELECT m.ROWID, m.guid, m.text, m.date, m.is_from_me,
                    h.id as handle_address, c.guid as chat_guid
             FROM message m
             LEFT JOIN handle h ON m.handle_id = h.ROWID
             LEFT JOIN chat_message_join cmj ON m.ROWID = cmj.message_id
             LEFT JOIN chat c ON cmj.chat_id = c.ROWID
             WHERE m.ROWID = ?1",
            params![rowid_int],
            |row| {
                let guid: String = row.get(1)?;
                let text: Option<String> = row.get(2)?;
                let date: i64 = row.get(3)?;
                let is_from_me: bool = row.get(4)?;
                let handle: Option<String> = row.get(5)?;
                let chat_guid: Option<String> = row.get(6)?;

                Ok(Message {
                    id: rowid.to_string(),
                    guid,
                    conversation_id: chat_guid.unwrap_or_default(),
                    sender: if is_from_me { "me".to_string() } else { handle.unwrap_or_default() },
                    body: text.unwrap_or_default(),
                    attachments: vec![],
                    timestamp: Self::mac_to_utc(date),
                    is_from_me,
                    status: if is_from_me { MessageStatus::Sent } else { MessageStatus::Delivered },
                })
            }
        ).map_err(|e| format!("Message not found: {}", e))
    }

    /// Get the guid for a message by ROWID
    pub fn guid_for_rowid(&self, rowid: &str) -> Result<String, String> {
        let rowid_int: i64 = rowid.parse().map_err(|_| format!("Invalid message id: {}", rowid))?;
        self.conn.query_row(
            "SELECT guid FROM message WHERE ROWID = ?1",
            params![rowid_int],
            |row| row.get(0),
        ).map_err(|e| format!("Message not found: {}", e))
    }

    /// Get conversations
    pub fn get_conversations(&self, query: &PaginationQuery) -> Result<Vec<Conversation>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT c.guid, c.display_name, c.chat_identifier
             FROM chat c
             ORDER BY c.ROWID DESC
             LIMIT ?1 OFFSET ?2"
        ).map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt.query_map(params![query.limit, query.offset], |row| {
            let guid: String = row.get(0)?;
            let display_name: Option<String> = row.get(1)?;
            let _chat_identifier: Option<String> = row.get(2)?;
            Ok((guid, display_name))
        }).map_err(|e| format!("Failed to query conversations: {}", e))?;

        let mut conversations = Vec::new();
        for row in rows {
            let (guid, display_name) = row.map_err(|e| format!("Row error: {}", e))?;
            let participants = self.get_chat_participants(&guid)?;
            let is_group = participants.len() > 1;
            conversations.push(Conversation {
                id: guid,
                participants,
                display_name,
                is_group,
                latest_message: None, // Could populate via subquery, skip for now
            });
        }
        Ok(conversations)
    }

    /// Get a single conversation by chat guid
    pub fn get_conversation(&self, chat_guid: &str) -> Result<Conversation, String> {
        self.conn.query_row(
            "SELECT guid, display_name FROM chat WHERE guid = ?1",
            params![chat_guid],
            |row| {
                let guid: String = row.get(0)?;
                let display_name: Option<String> = row.get(1)?;
                Ok((guid, display_name))
            }
        ).map_err(|e| format!("Conversation not found: {}", e))
        .and_then(|(guid, display_name)| {
            let participants = self.get_chat_participants(&guid)?;
            let is_group = participants.len() > 1;
            Ok(Conversation {
                id: guid,
                participants,
                display_name,
                is_group,
                latest_message: None,
            })
        })
    }

    fn get_chat_participants(&self, chat_guid: &str) -> Result<Vec<String>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT h.id FROM handle h
             JOIN chat_handle_join chj ON h.ROWID = chj.handle_id
             JOIN chat c ON chj.chat_id = c.ROWID
             WHERE c.guid = ?1"
        ).map_err(|e| format!("Failed to get participants: {}", e))?;

        let rows = stmt.query_map(params![chat_guid], |row| {
            row.get::<_, String>(0)
        }).map_err(|e| format!("Failed to read participants: {}", e))?;

        let mut participants = Vec::new();
        for row in rows {
            participants.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(participants)
    }

    /// Find a recently sent message matching recipient + body (for post-send lookup)
    pub fn find_sent_message(&self, recipient: &str, body: &str) -> Result<Option<Message>, String> {
        // Look for a recent is_from_me=1 message to this recipient with matching body
        let result = self.conn.query_row(
            "SELECT m.ROWID, m.guid, m.text, m.date,
                    c.guid as chat_guid
             FROM message m
             LEFT JOIN chat_message_join cmj ON m.ROWID = cmj.message_id
             LEFT JOIN chat c ON cmj.chat_id = c.ROWID
             LEFT JOIN chat_handle_join chj ON c.ROWID = chj.chat_id
             LEFT JOIN handle h ON chj.handle_id = h.ROWID
             WHERE m.is_from_me = 1 AND m.text = ?1 AND h.id = ?2
             ORDER BY m.ROWID DESC LIMIT 1",
            params![body, recipient],
            |row| {
                let rowid: i64 = row.get(0)?;
                let guid: String = row.get(1)?;
                let text: Option<String> = row.get(2)?;
                let date: i64 = row.get(3)?;
                let chat_guid: Option<String> = row.get(4)?;

                Ok(Message {
                    id: rowid.to_string(),
                    guid,
                    conversation_id: chat_guid.unwrap_or_default(),
                    sender: "me".to_string(),
                    body: text.unwrap_or_default(),
                    attachments: vec![],
                    timestamp: Self::mac_to_utc(date),
                    is_from_me: true,
                    status: MessageStatus::Sent,
                })
            }
        );

        match result {
            Ok(msg) => Ok(Some(msg)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Failed to find sent message: {}", e)),
        }
    }

    /// Get the current highest ROWID in chat.db
    pub fn get_max_rowid(&self) -> Result<i64, String> {
        self.conn.query_row(
            "SELECT MAX(ROWID) FROM message",
            [],
            |row| row.get::<_, Option<i64>>(0).map(|v| v.unwrap_or(0)),
        ).map_err(|e| format!("Failed to get max ROWID: {}", e))
    }
}
