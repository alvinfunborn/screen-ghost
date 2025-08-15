use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct FaceConfig {
    pub detection: DetectionConfig,
    pub recognition: RecognitionConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct DetectionConfig {
    pub min_face_size: i32,
    pub max_face_size: i32,
    pub scale_factor: f64,
    pub min_neighbors: i32,
    pub confidence_threshold: f32,
    pub use_gray: bool,
    pub image_scale: f32,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct RecognitionConfig {
    pub threshold: f32,
}
