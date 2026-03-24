use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

pub struct Storage {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WebhookRecord {
    pub id: String,
    pub url: String,
    pub events: Vec<String>,
    pub secret: Option<String>,
    pub created_at: String,
}

impl Storage {
    pub fn new(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let storage = Storage {
            conn: Mutex::new(conn),
        };
        storage.run_migrations()?;
        Ok(storage)
    }

    fn run_migrations(&self) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS webhooks (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                events TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_webhooks_url ON webhooks(url);

            CREATE TABLE IF NOT EXISTS message_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                imessage_rowid TEXT NOT NULL UNIQUE,
                conversation_id TEXT NOT NULL,
                delivered_at TEXT NOT NULL DEFAULT (datetime('now')),
                webhook_delivery_status TEXT NOT NULL DEFAULT 'pending'
            );

            CREATE TABLE IF NOT EXISTS state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS webhook_deliveries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                imessage_rowid TEXT NOT NULL,
                webhook_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                delivered_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(imessage_rowid, webhook_id)
            );

"
        ).map_err(|e| format!("Migration failed: {}", e))?;
        // Add secret column if it doesn't exist (idempotent)
        let _ = conn.execute("ALTER TABLE webhooks ADD COLUMN secret TEXT", []);
        Ok(())
    }

    pub fn create_or_update_webhook(&self, url: &str, events: &[String], secret: Option<&str>) -> Result<WebhookRecord, String> {
        let conn = self.conn.lock().unwrap();
        let events_json = serde_json::to_string(events).unwrap();

        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM webhooks WHERE url = ?1",
                params![url],
                |row| row.get(0),
            )
            .ok();

        let id = if let Some(existing_id) = existing {
            conn.execute(
                "UPDATE webhooks SET events = ?1, secret = ?2 WHERE id = ?3",
                params![events_json, secret, existing_id],
            )
            .map_err(|e| format!("Failed to update webhook: {}", e))?;
            existing_id
        } else {
            let new_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO webhooks (id, url, events, secret) VALUES (?1, ?2, ?3, ?4)",
                params![new_id, url, events_json, secret],
            )
            .map_err(|e| format!("Failed to create webhook: {}", e))?;
            new_id
        };

        // Must drop conn before calling get_webhook (which also locks)
        drop(conn);
        self.get_webhook(&id)
    }

    pub fn get_webhook(&self, id: &str) -> Result<WebhookRecord, String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, url, events, secret, created_at FROM webhooks WHERE id = ?1",
            params![id],
            |row| {
                let events_str: String = row.get(2)?;
                let events: Vec<String> = serde_json::from_str(&events_str).unwrap_or_default();
                Ok(WebhookRecord {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    events,
                    secret: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        )
        .map_err(|e| format!("Webhook not found: {}", e))
    }

    pub fn list_webhooks(&self) -> Result<Vec<WebhookRecord>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, url, events, secret, created_at FROM webhooks")
            .map_err(|e| format!("Failed to query webhooks: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                let events_str: String = row.get(2)?;
                let events: Vec<String> = serde_json::from_str(&events_str).unwrap_or_default();
                Ok(WebhookRecord {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    events,
                    secret: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .map_err(|e| format!("Failed to read webhooks: {}", e))?;

        let mut webhooks = Vec::new();
        for row in rows {
            webhooks.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(webhooks)
    }

    pub fn delete_webhook(&self, id: &str) -> Result<bool, String> {
        let conn = self.conn.lock().unwrap();
        let affected = conn
            .execute("DELETE FROM webhooks WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete webhook: {}", e))?;
        Ok(affected > 0)
    }

    pub fn get_webhooks_for_event(&self, event: &str) -> Result<Vec<WebhookRecord>, String> {
        let all = self.list_webhooks()?;
        Ok(all.into_iter().filter(|w| w.events.contains(&event.to_string())).collect())
    }

    pub fn log_message(
        &self,
        imessage_rowid: &str,
        conversation_id: &str,
    ) -> Result<bool, String> {
        let conn = self.conn.lock().unwrap();
        let result = conn.execute(
            "INSERT OR IGNORE INTO message_log (imessage_rowid, conversation_id) VALUES (?1, ?2)",
            params![imessage_rowid, conversation_id],
        );
        match result {
            Ok(count) => Ok(count > 0),
            Err(e) => Err(format!("Failed to log message: {}", e)),
        }
    }

    pub fn log_delivery(
        &self,
        imessage_rowid: &str,
        webhook_id: &str,
    ) -> Result<bool, String> {
        let conn = self.conn.lock().unwrap();
        let result = conn.execute(
            "INSERT OR IGNORE INTO webhook_deliveries (imessage_rowid, webhook_id) VALUES (?1, ?2)",
            params![imessage_rowid, webhook_id],
        );
        match result {
            Ok(count) => Ok(count > 0),
            Err(e) => Err(format!("Failed to log delivery: {}", e)),
        }
    }

    pub fn update_delivery_status(
        &self,
        imessage_rowid: &str,
        webhook_id: &str,
        status: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE webhook_deliveries SET status = ?1 WHERE imessage_rowid = ?2 AND webhook_id = ?3",
            params![status, imessage_rowid, webhook_id],
        )
        .map_err(|e| format!("Failed to update delivery status: {}", e))?;
        Ok(())
    }

    pub fn get_state(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM state WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Failed to get state: {}", e))
    }

    pub fn set_state(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO state (key, value) VALUES (?1, ?2)",
            params![key, value],
        )
        .map_err(|e| format!("Failed to set state: {}", e))?;
        Ok(())
    }

    pub fn get_last_rowid(&self) -> Result<i64, String> {
        self.get_state("last_processed_rowid")
            .map(|v| v.and_then(|s| s.parse().ok()).unwrap_or(0))
    }

    pub fn set_last_rowid(&self, rowid: i64) -> Result<(), String> {
        self.set_state("last_processed_rowid", &rowid.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_storage() -> Storage {
        Storage::new(std::path::Path::new(":memory:")).unwrap()
    }

    #[test]
    fn test_create_and_list_webhooks() {
        let s = test_storage();
        let wh = s.create_or_update_webhook("http://test.com/hook", &["message.received".into()], None).unwrap();
        assert!(!wh.id.is_empty());
        assert_eq!(wh.url, "http://test.com/hook");
        assert_eq!(wh.events, vec!["message.received"]);

        let list = s.list_webhooks().unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_duplicate_url_updates() {
        let s = test_storage();
        let wh1 = s.create_or_update_webhook("http://test.com/hook", &["message.received".into()], None).unwrap();
        let wh2 = s.create_or_update_webhook("http://test.com/hook", &["message.sent".into()], None).unwrap();
        assert_eq!(wh1.id, wh2.id);
        assert_eq!(wh2.events, vec!["message.sent"]);
        assert_eq!(s.list_webhooks().unwrap().len(), 1);
    }

    #[test]
    fn test_delete_webhook() {
        let s = test_storage();
        let wh = s.create_or_update_webhook("http://test.com/hook", &["message.received".into()], None).unwrap();
        assert!(s.delete_webhook(&wh.id).unwrap());
        assert!(!s.delete_webhook(&wh.id).unwrap());
        assert_eq!(s.list_webhooks().unwrap().len(), 0);
    }

    #[test]
    fn test_webhook_secret() {
        let s = test_storage();
        let wh = s.create_or_update_webhook("http://test.com/hook", &["message.received".into()], Some("mysecret")).unwrap();
        assert_eq!(wh.secret, Some("mysecret".to_string()));
    }

    #[test]
    fn test_get_webhooks_for_event() {
        let s = test_storage();
        s.create_or_update_webhook("http://a.com", &["message.received".into()], None).unwrap();
        s.create_or_update_webhook("http://b.com", &["message.sent".into()], None).unwrap();
        s.create_or_update_webhook("http://c.com", &["message.received".into(), "message.sent".into()], None).unwrap();

        let received = s.get_webhooks_for_event("message.received").unwrap();
        assert_eq!(received.len(), 2);
        let sent = s.get_webhooks_for_event("message.sent").unwrap();
        assert_eq!(sent.len(), 2);
    }

    #[test]
    fn test_message_dedup() {
        let s = test_storage();
        assert!(s.log_message("rowid1", "conv1").unwrap());
        assert!(!s.log_message("rowid1", "conv1").unwrap());
    }

    #[test]
    fn test_state_persistence() {
        let s = test_storage();
        assert_eq!(s.get_last_rowid().unwrap(), 0);
        s.set_last_rowid(12345).unwrap();
        assert_eq!(s.get_last_rowid().unwrap(), 12345);
        s.set_last_rowid(99999).unwrap();
        assert_eq!(s.get_last_rowid().unwrap(), 99999);
    }

    #[test]
    fn test_delivery_tracking() {
        let s = test_storage();
        assert!(s.log_delivery("rowid1", "webhook1").unwrap());
        assert!(!s.log_delivery("rowid1", "webhook1").unwrap());
        assert!(s.log_delivery("rowid1", "webhook2").unwrap());
        s.update_delivery_status("rowid1", "webhook1", "delivered").unwrap();
        s.update_delivery_status("rowid1", "webhook2", "failed").unwrap();
    }
}
