use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mosaic {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}