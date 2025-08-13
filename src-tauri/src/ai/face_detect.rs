use crate::{monitor::Image, utils::rect::Rect};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use rayon::prelude::*;
use std::sync::OnceLock;
use crate::ai::python_env;

// 全局Python初始化状态，避免重复初始化
static PYTHON_INITIALIZED: OnceLock<bool> = OnceLock::new();

// 配置参数
const MIN_FACE_SIZE: i32 = 30;
const MAX_FACE_SIZE: i32 = 300;
const SCALE_FACTOR: f64 = 1.1;
const MIN_NEIGHBORS: i32 = 3;

#[derive(Debug, Clone)]
pub struct FaceDetectionConfig {
    pub min_face_size: i32,
    pub max_face_size: i32,
    pub scale_factor: f64,
    pub min_neighbors: i32,
    pub confidence_threshold: f32,
}

impl Default for FaceDetectionConfig {
    fn default() -> Self {
        Self {
            min_face_size: MIN_FACE_SIZE,
            max_face_size: MAX_FACE_SIZE,
            scale_factor: SCALE_FACTOR,
            min_neighbors: MIN_NEIGHBORS,
            confidence_threshold: 0.5,
        }
    }
}

pub fn face_detect(image: &Image) -> Result<Vec<Rect>, String> {
    face_detect_with_config(image, &FaceDetectionConfig::default())
}

pub fn face_detect_with_config(
    image: &Image,
    config: &FaceDetectionConfig,
) -> Result<Vec<Rect>, String> {
    let start_time = std::time::Instant::now();
    
    // 调用Python进行人脸检测
    let faces = call_python_face_detection(image)?;
    
    // 转换坐标系统
    let rects = convert_to_rects(faces, image.width, image.height);
    
    let elapsed = start_time.elapsed();
    log::info!("[face_detect] Detection completed in {:?}, found {} faces", elapsed, rects.len());
    
    Ok(rects)
}

fn call_python_face_detection(image: &Image) -> Result<Vec<(i32, i32, i32, i32)>, String> {
    // 确保Python环境已初始化
    ensure_python_initialized()?;
    
    Python::with_gil(|py| {
        // 获取Python文件路径
        let python_files_path = python_env::get_python_files_path()
            .map_err(|e| format!("Failed to get python files path: {}", e))?;
        
        // 设置Python路径
        let path_setup = format!(
            r#"
import sys
import os
sys.path.insert(0, r'{}')
"#,
            python_files_path.to_string_lossy()
        );
        
        py.run(&path_setup, None, None)
            .map_err(|e| format!("Failed to setup Python path: {}", e))?;
        
        // 导入Python模块
        let face_detection = py.import("face_detection")
            .map_err(|e| format!("Failed to import face_detection module: {}", e))?;
        
        // 调用Python函数
        let result: Vec<(i32, i32, i32, i32)> = face_detection
            .call_method1("detect_faces", (PyBytes::new(py, &image.data), image.width, image.height))
            .map_err(|e| format!("Failed to call Python function: {}", e))?
            .extract()
            .map_err(|e| format!("Failed to extract face detection result: {}", e))?;
        
        Ok(result)
    })
}

fn ensure_python_initialized() -> Result<(), String> {
    // 检查是否已经初始化
    if let Some(&initialized) = PYTHON_INITIALIZED.get() {
        if initialized {
            return Ok(());
        }
    }
    
    // 使用新的Python环境管理器
    python_env::initialize_python_environment()?;
    
    // 验证Python环境是否可用
    if !python_env::is_python_ready() {
        return Err("Python environment is not ready. Please check the installation guide.".to_string());
    }
    
    // 使用系统Python，不设置特殊的环境变量
    // 让PyO3使用默认的系统Python环境
    log::info!("Using system Python environment");
    
    // 初始化PyO3
    let result = Python::with_gil(|py| {
        // 检查Python环境
        let sys = py.import("sys")?;
        let version: String = sys.getattr("version")?.extract()?;
        let executable: String = sys.getattr("executable")?.extract()?;
        log::info!("Python version: {}", version);
        log::info!("Python executable: {}", executable);
        
        // 检查必要的包
        let required_packages = ["cv2", "numpy"];
        for package in required_packages {
            if let Err(e) = py.import(package) {
                return Err(PyErr::new::<pyo3::exceptions::PyImportError, _>(
                    format!("Required package '{}' not found: {}", package, e)
                ));
            }
        }
        
        Ok(())
    });
    
    match result {
        Ok(_) => {
            // 标记为已初始化
            PYTHON_INITIALIZED.set(true).map_err(|_| "Failed to set initialization flag".to_string())?;
            Ok(())
        }
        Err(e) => Err(format!("Failed to initialize Python interpreter: {}", e))
    }
}

fn convert_to_rects(faces: Vec<(i32, i32, i32, i32)>, image_width: i32, image_height: i32) -> Vec<Rect> {
    faces
        .into_iter()
        .map(|(x, y, width, height)| {
            // 确保坐标在图像范围内
            let x = x.max(0).min(image_width - width);
            let y = y.max(0).min(image_height - height);
            let width = width.min(image_width - x);
            let height = height.min(image_height - y);
            
            Rect::new(x, y, width, height)
        })
        .collect()
}

// 高性能批处理版本
pub fn face_detect_batch(images: &[&Image]) -> Result<Vec<Vec<Rect>>, String> {
    let config = FaceDetectionConfig::default();
    
    // 并行处理多个图像
    let results: Vec<Result<Vec<Rect>, String>> = images
        .par_iter()
        .map(|image| face_detect_with_config(image, &config))
        .collect();
    
    // 收集结果
    let mut batch_results = Vec::new();
    for result in results {
        match result {
            Ok(faces) => batch_results.push(faces),
            Err(e) => return Err(format!("Batch detection failed: {}", e)),
        }
    }
    
    Ok(batch_results)
}

// 高性能人脸检测 - 优化版本
pub fn face_detect_high_performance(image: &Image) -> Result<Vec<Rect>, String> {
    let start_time = std::time::Instant::now();
    
    // 调用Python高性能版本
    let faces = call_python_high_performance_detection(image)?;
    
    // 转换坐标系统
    let rects = convert_to_rects(faces, image.width, image.height);
    
    let elapsed = start_time.elapsed();
    log::info!("[face_detect_high_performance] Detection completed in {:?}, found {} faces", elapsed, rects.len());
    
    Ok(rects)
}

fn call_python_high_performance_detection(image: &Image) -> Result<Vec<(i32, i32, i32, i32)>, String> {
    // 确保Python环境已初始化
    ensure_python_initialized()?;
    
    Python::with_gil(|py| {
        // 获取Python文件路径
        let python_files_path = python_env::get_python_files_path()
            .map_err(|e| format!("Failed to get python files path: {}", e))?;
        
        // 设置Python路径
        let path_setup = format!(
            r#"
import sys
import os
sys.path.insert(0, r'{}')
"#,
            python_files_path.to_string_lossy()
        );
        
        py.run(&path_setup, None, None)
            .map_err(|e| format!("Failed to setup Python path: {}", e))?;
        
        // 导入Python模块
        let face_detection = py.import("face_detection")
            .map_err(|e| format!("Failed to import face_detection module: {}", e))?;
        
        // 调用Python高性能函数
        let result: Vec<(i32, i32, i32, i32)> = face_detection
            .call_method1("detect_faces_high_performance", (PyBytes::new(py, &image.data), image.width, image.height))
            .map_err(|e| format!("Failed to call Python function: {}", e))?
            .extract()
            .map_err(|e| format!("Failed to extract high performance face detection result: {}", e))?;
        
        Ok(result)
    })
}

// 实时检测优化版本
pub fn face_detect_realtime(image: &Image) -> Result<Vec<Rect>, String> {
    face_detect_high_performance(image)
}

// 初始化Python环境
pub fn initialize_python_environment() -> Result<(), String> {
    log::info!("Initializing Python environment for face detection...");
    
    // 使用新的Python环境管理器
    python_env::initialize_python_environment()?;
    
    if !python_env::is_python_ready() {
        let guide = python_env::get_installation_guide();
        log::error!("Python environment is not ready. Installation guide:\n{}", guide);
        return Err("Python environment is not ready. Please check the logs for installation guide.".to_string());
    }
    
    // 验证PyO3环境
    let result: Result<(), PyErr> = Python::with_gil(|py| {
        // 检查Python版本
        let sys = py.import("sys")?;
        let version: String = sys.getattr("version")?.extract()?;
        log::info!("Python version: {}", version);
        
        // 检查OpenCV
        let cv2 = py.import("cv2")?;
        let cv_version: String = cv2.getattr("__version__")?.extract()?;
        log::info!("OpenCV version: {}", cv_version);
        
        // 检查numpy
        let np = py.import("numpy")?;
        let np_version: String = np.getattr("__version__")?.extract()?;
        log::info!("NumPy version: {}", np_version);
        
        Ok(())
    });
    
    match result {
        Ok(_) => {
            log::info!("Python environment initialized successfully");
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to initialize Python environment: {}", e);
            Err(format!("Python environment initialization failed: {}", e))
        }
    }
}

// 初始化Python环境（带应用句柄）
pub fn initialize_python_environment_with_app_handle(app_handle: &tauri::AppHandle) -> Result<(), String> {
    log::info!("Initializing Python environment for face detection...");
    
    // 使用新的Python环境管理器
    python_env::initialize_python_environment_with_app_handle(app_handle)?;
    
    if !python_env::is_python_ready() {
        let guide = python_env::get_installation_guide();
        log::error!("Python environment is not ready. Installation guide:\n{}", guide);
        return Err("Python environment is not ready. Please check the logs for installation guide.".to_string());
    }
    
    // 验证PyO3环境
    let result: Result<(), PyErr> = Python::with_gil(|py| {
        // 检查Python版本
        let sys = py.import("sys")?;
        let version: String = sys.getattr("version")?.extract()?;
        log::info!("Python version: {}", version);
        
        // 检查OpenCV
        let cv2 = py.import("cv2")?;
        let cv_version: String = cv2.getattr("__version__")?.extract()?;
        log::info!("OpenCV version: {}", cv_version);
        
        // 检查numpy
        let np = py.import("numpy")?;
        let np_version: String = np.getattr("__version__")?.extract()?;
        log::info!("NumPy version: {}", np_version);
        
        Ok(())
    });
    
    match result {
        Ok(_) => {
            log::info!("Python environment initialized successfully");
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to initialize Python environment: {}", e);
            Err(format!("Python environment initialization failed: {}", e))
        }
    }
}