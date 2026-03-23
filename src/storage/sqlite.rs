use rusqlite::{params, Connection};
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
                backend_message_id TEXT NOT NULL UNIQUE,
                conversation_id TEXT NOT NULL,
                delivered_at TEXT NOT NULL DEFAULT (datetime('now')),
                webhook_delivery_status TEXT NOT NULL DEFAULT 'pending'
            );"
        ).map_err(|e| format!("Migration failed: {}", e))?;
        Ok(())
    }

    pub fn create_or_update_webhook(&self, url: &str, events: &[String]) -> Result<WebhookRecord, String> {
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
                "UPDATE webhooks SET events = ?1 WHERE id = ?2",
                params![events_json, existing_id],
            )
            .map_err(|e| format!("Failed to update webhook: {}", e))?;
            existing_id
        } else {
            let new_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO webhooks (id, url, events) VALUES (?1, ?2, ?3)",
                params![new_id, url, events_json],
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
            "SELECT id, url, events, created_at FROM webhooks WHERE id = ?1",
            params![id],
            |row| {
                let events_str: String = row.get(2)?;
                let events: Vec<String> = serde_json::from_str(&events_str).unwrap_or_default();
                Ok(WebhookRecord {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    events,
                    created_at: row.get(3)?,
                })
            },
        )
        .map_err(|e| format!("Webhook not found: {}", e))
    }

    pub fn list_webhooks(&self) -> Result<Vec<WebhookRecord>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, url, events, created_at FROM webhooks")
            .map_err(|e| format!("Failed to query webhooks: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                let events_str: String = row.get(2)?;
                let events: Vec<String> = serde_json::from_str(&events_str).unwrap_or_default();
                Ok(WebhookRecord {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    events,
                    created_at: row.get(3)?,
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
        backend_message_id: &str,
        conversation_id: &str,
    ) -> Result<bool, String> {
        let conn = self.conn.lock().unwrap();
        let result = conn.execute(
            "INSERT OR IGNORE INTO message_log (backend_message_id, conversation_id) VALUES (?1, ?2)",
            params![backend_message_id, conversation_id],
        );
        match result {
            Ok(count) => Ok(count > 0),
            Err(e) => Err(format!("Failed to log message: {}", e)),
        }
    }

    pub fn update_delivery_status(
        &self,
        backend_message_id: &str,
        status: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE message_log SET webhook_delivery_status = ?1 WHERE backend_message_id = ?2",
            params![status, backend_message_id],
        )
        .map_err(|e| format!("Failed to update status: {}", e))?;
        Ok(())
    }
}
