use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct FaceConfig {
    pub detection: DetectionConfig,
    pub recognition: RecognitionConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct DetectionConfig {
    pub min_face_size: Option<i32>,
    pub max_face_size: Option<i32>,
    // 可选：按短边比例指定人脸最小/最大尺寸（0.0~1.0）。若提供，则优先于 *_face_size。
    pub min_face_ratio: Option<f32>,
    pub max_face_ratio: Option<f32>,
    pub scale_factor: f64,
    pub min_neighbors: i32,
    pub confidence_threshold: f32,
    pub use_gray: bool,
    pub image_scale: f32,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct RecognitionConfig {
    pub threshold: f32,
    pub provider: Option<String>,
    pub outlier_threshold: Option<f32>,
    pub outlier_iter: Option<i32>,
}
