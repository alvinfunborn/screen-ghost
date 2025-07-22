use crate::{monitor::Image, utils::rect::Rect};

pub fn face_detect(image: &Image) -> Result<Vec<Rect>, String> {
    // 人脸检测
    // 1. 将image转换为opencv的Mat
    let mat = Mat::from_image(image);
    // 2. 使用opencv的人脸检测模型进行检测
    let faces = face_detect_model.detect(mat);
    // 4. 返回检测结果
    Ok(faces)
}