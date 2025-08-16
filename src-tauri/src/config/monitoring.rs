use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct MonitoringConfig {
    pub interval: u64,
    pub mosaic_scale: f32,
    pub mosaic_style: String,
}
