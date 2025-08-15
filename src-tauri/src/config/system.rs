use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct SystemConfig {
    pub log_level: Option<String>,
}