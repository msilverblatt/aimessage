use crate::core_layer::errors::BackendError;
use crate::core_layer::types::ReactionType;

pub struct PrivateApi {
    available: bool,
}

impl PrivateApi {
    pub fn new(enabled: bool) -> Self {
        let available = if enabled {
            Self::check_availability()
        } else {
            false
        };

        if enabled && !available {
            tracing::warn!("Private API enabled in config but not available. SIP may not be disabled.");
        } else if available {
            tracing::info!("Private API available — reactions and typing indicators enabled");
        }

        PrivateApi { available }
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    fn check_availability() -> bool {
        let output = std::process::Command::new("csrutil")
            .arg("status")
            .output();

        match output {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                stdout.contains("disabled")
            }
            Err(_) => false,
        }
    }

    pub fn send_reaction(&self, _message_guid: &str, _reaction: &ReactionType) -> Result<(), BackendError> {
        if !self.available {
            return Err(BackendError::FeatureUnavailable(
                "Private API not available. Disable SIP and set private_api = true in config.".to_string(),
            ));
        }
        Err(BackendError::FeatureUnavailable(
            "Private API reaction sending not yet implemented.".to_string(),
        ))
    }

    pub fn send_typing(&self, _chat_guid: &str) -> Result<(), BackendError> {
        if !self.available {
            return Err(BackendError::FeatureUnavailable(
                "Private API not available. Disable SIP and set private_api = true in config.".to_string(),
            ));
        }
        Err(BackendError::FeatureUnavailable(
            "Private API typing indicator not yet implemented.".to_string(),
        ))
    }
}
