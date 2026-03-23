use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub backend: BackendConfig,
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
pub struct BackendConfig {
    #[serde(rename = "type")]
    pub backend_type: String,
    pub bluebubbles: Option<BlueBubblesConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueBubblesConfig {
    pub url: String,
    pub password: String,
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

        match self.backend.backend_type.as_str() {
            "bluebubbles" => {
                let bb = self.backend.bluebubbles.as_ref().ok_or(
                    "backend.type is 'bluebubbles' but [backend.bluebubbles] section is missing"
                        .to_string(),
                )?;
                if bb.password.is_empty() || bb.password == "CHANGE_ME" {
                    return Err(
                        "BlueBubbles password not configured. Edit ~/.aimessage/config.toml"
                            .to_string(),
                    );
                }
            }
            other => return Err(format!("Unknown backend type: {}", other)),
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
            backend: BackendConfig {
                backend_type: "bluebubbles".to_string(),
                bluebubbles: Some(BlueBubblesConfig {
                    url: "http://localhost:1234".to_string(),
                    password: "CHANGE_ME".to_string(),
                }),
            },
        };

        let dir = path.parent().unwrap();
        fs::create_dir_all(dir).expect("Failed to create config directory");
        let content = toml::to_string_pretty(&default).unwrap();
        fs::write(path, &content).expect("Failed to write default config");

        format!(
            "Generated default config at {}.\nYour API key: {}\nEdit the file to set your BlueBubbles password, then restart.",
            path.display(),
            api_key
        )
    }
}
