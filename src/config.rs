use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub imessage: IMessageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IMessageConfig {
    pub chat_db_path: String,
    pub poll_interval_ms: u64,
    pub private_api: bool,
}

impl Config {
    pub fn config_dir() -> PathBuf {
        dirs::home_dir()
            .expect("Could not determine home directory")
            .join(".aimessage")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn default_chat_db_path() -> String {
        dirs::home_dir()
            .expect("Could not determine home directory")
            .join("Library/Messages/chat.db")
            .to_string_lossy()
            .to_string()
    }

    pub fn load() -> Result<Self, String> {
        let path = Self::config_path();

        if !path.exists() {
            return Err(Self::generate_default(&path));
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read config at {}: {}", path.display(), e))?;

        let config: Config = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse config: {}", e))?;

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        if self.auth.api_key == "CHANGE_ME" || self.auth.api_key.is_empty() {
            return Err("API key not configured. Edit ~/.aimessage/config.toml".to_string());
        }

        let db_path = Path::new(&self.imessage.chat_db_path);
        if !db_path.exists() {
            return Err(format!(
                "chat.db not found at {}.\n\
                 This usually means Full Disk Access is not granted.\n\
                 Go to: System Settings → Privacy & Security → Full Disk Access\n\
                 Add your terminal or the aimessage binary.",
                self.imessage.chat_db_path
            ));
        }

        Ok(())
    }

    fn generate_default(path: &Path) -> String {
        let api_key = uuid::Uuid::new_v4().to_string();

        let default = Config {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 3001,
            },
            auth: AuthConfig {
                api_key: api_key.clone(),
            },
            imessage: IMessageConfig {
                chat_db_path: Self::default_chat_db_path(),
                poll_interval_ms: 1000,
                private_api: false,
            },
        };

        let dir = path.parent().unwrap();
        fs::create_dir_all(dir).expect("Failed to create config directory");
        let content = toml::to_string_pretty(&default).unwrap();
        fs::write(path, &content).expect("Failed to write default config");

        format!(
            "Generated default config at {}.\nYour API key: {}\nThe server will auto-detect your iMessage database. Just restart.",
            path.display(),
            api_key
        )
    }
}
