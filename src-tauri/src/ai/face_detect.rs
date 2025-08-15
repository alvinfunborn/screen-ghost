use crate::{config::{self, DetectionConfig}, monitor::Image, utils::rect::Rect};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use std::sync::OnceLock;
use crate::ai::python_env;

// 全局Python初始化状态，避免重复初始化
static PYTHON_INITIALIZED: OnceLock<bool> = OnceLock::new();

pub fn face_detect(image: &Image) -> Result<Vec<Rect>, String> {
    let cfg = config::get_config().unwrap().face.unwrap().detection;
    face_detect_with_config(image, &cfg)
}

pub fn face_detect_with_config(
    image: &Image,
    config: &DetectionConfig,
) -> Result<Vec<Rect>, String> {
    let start_time = std::time::Instant::now();
    
    // 统一调用：由配置驱动（不再区分多管道）
    let faces = call_python_face_detection_with_config(image, config)?;
    
    // 转换坐标系统
    let rects = convert_to_rects(faces, image.width, image.height);
    
    let elapsed = start_time.elapsed();
    log::debug!("[face_detect] Detection completed in {:?}, found {} faces (gray: {}, scale: {})", 
                elapsed, rects.len(), config.use_gray, config.scale_factor);
    
    Ok(rects)
}

// 新增：灰度图像检测
// 统一管道：通过 FaceDetectionConfig 配置 use_gray 与 image_scale 达到性能/实时效果

// 移除未使用的旧接口 call_python_face_detection

fn call_python_face_detection_with_config(
    image: &Image,
    config: &DetectionConfig,
) -> Result<Vec<(i32, i32, i32, i32)>, String> {
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

        // 清理可能的旧缓存并刷新查找，避免加载到系统里同名模块
        let _ = py.run(
            "import sys, importlib; sys.modules.pop('face_detection', None); importlib.invalidate_caches()",
            None,
            None,
        );

        // 导入并尽量刷新Python模块，避免旧缓存
        let face_detection = {
            // 先尝试普通导入
            let module = py.import("face_detection")
                .map_err(|e| format!("Failed to import face_detection module: {}", e))?;
            // 尝试 importlib.reload，确保最新脚本生效（忽略 reload 失败）
            let locals = PyDict::new(py);
            let _ = locals.set_item("face_detection", module);
            let _ = py.run(
                r#"
import importlib
try:
    importlib.reload(face_detection)
except Exception:
    pass
"#,
                None,
                Some(locals),
            );
            // 记录模块来源路径，便于诊断是否加载到正确的脚本
            if let Ok(file_attr) = module.getattr("__file__") {
                if let Ok(py_str) = file_attr.str() {
                    if let Ok(path) = py_str.to_str() {
                        log::info!("[face_detect] loaded face_detection from: {}", path);
                    }
                }
            }
            module
        };

        // 优先调用统一配置函数；若不存在则回退到旧版 detect_faces
        let call_args = (
            PyBytes::new(py, &image.data),
            image.width,
            image.height,
            config.use_gray,
            config.image_scale,
            config.min_face_size,
            config.max_face_size,
            config.scale_factor,
            config.min_neighbors,
            config.confidence_threshold,
        );

        let result: Vec<(i32, i32, i32, i32)> = match face_detection.call_method1("detect_faces_with_config", call_args) {
            Ok(obj) => obj
                .extract()
                .map_err(|e| format!("Failed to extract detect_faces_with_config result: {}", e))?,
            Err(e) => {
                // 如果方法不存在或调用失败，尝试回退到老接口
                log::warn!("detect_faces_with_config unavailable ({}), fallback to detect_faces", e);
                face_detection
                    .call_method1(
                        "detect_faces",
                        (PyBytes::new(py, &image.data), image.width, image.height),
                    )
                    .map_err(|e2| format!("Failed to call fallback detect_faces: {}", e2))?
                    .extract()
                    .map_err(|e2| format!("Failed to extract fallback detect_faces result: {}", e2))?
            }
        };

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
    // 确保进程环境包含所选 Python 的目录（Windows: PATH/PYTHONHOME 等）
    python_env::PythonEnvManager::get_instance().prepare_process_env();
    
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

// —— 统一化后，移除了批量/实时/高性能等多管道的公开入口 ——

// 新增：调用Python灰度检测函数
// 移除未使用的灰度辅助入口 call_python_face_detection_gray

// 新增：调用Python灰度检测函数（原始灰度数据）
// 移除未使用的灰度底层入口 call_python_face_detection_gray_raw

// 新增：图像转换为灰度
// 移除未使用的图像灰度转换辅助 convert_to_gray

// 初始化Python环境
#[allow(dead_code)]
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