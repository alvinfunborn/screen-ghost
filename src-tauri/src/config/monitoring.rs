use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct MonitoringConfig {
    pub interval: u64,
    pub mosaic_scale: f32,
    pub mosaic_style: String,
    // 可选：对截图做下采样（0.1~1.0），仅用于检测加速，遮罩坐标将自动还原到原分辨率
    pub capture_scale: Option<f32>,
}
