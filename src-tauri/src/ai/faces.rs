use crate::monitor::Image;
use crate::utils::rect::Rect;
use crate::ai::python_env;
use log::info;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

static FACE_MODEL_READY: OnceLock<AtomicBool> = OnceLock::new();

fn face_model_flag() -> &'static AtomicBool {
    FACE_MODEL_READY.get_or_init(|| AtomicBool::new(false))
}

pub fn is_face_model_ready() -> bool {
    face_model_flag().load(Ordering::SeqCst)
}

// 统一入口：若存在目标人脸库，则返回命中的最佳目标；否则返回所有检测人脸
pub fn detect_targets_or_all_faces(image: &Image) -> Result<Vec<Rect>, String> {
    // 统一委托给 Python faces.detect_targets_or_all_faces
    Python::with_gil(|py| {
        let python_files_path = python_env::get_python_files_path()
            .map_err(|e| format!("Failed to get python files path: {}", e))?;
        let path_setup = format!(
            r#"
import sys
import os
if r'{0}' not in sys.path:
    sys.path.insert(0, r'{0}')
"#,
            python_files_path.to_string_lossy()
        );
        py.run(&path_setup, None, None)
            .map_err(|e| format!("Failed to setup Python path: {}", e))?;
        // 优先从 python_files 导入；若失败或命名冲突导入到其他包，按路径兜底加载 faces.py
        let fallback_import = format!(
            r#"
import sys, os, importlib.util
module_name = 'faces'
try:
    import faces as mod
    # 若导入的 faces 不包含所需方法，视为命名冲突，按路径兜底
    _ok = hasattr(mod, 'detect_targets_or_all_faces') or hasattr(mod, 'init_model')
    if not _ok:
        raise ImportError('conflicting faces module without required attributes')
except Exception:
    bases = []
    # 应用数据目录（python_files）
    bases.append(r'{p}')
    try:
        exe_dir = os.path.dirname(sys.executable)
        bases.append(os.path.join(exe_dir, 'python'))
        bases.append(os.path.join(exe_dir, 'src-tauri', 'python'))
    except Exception:
        pass
    # 工作目录候选
    try:
        cwd = os.getcwd()
        bases.append(os.path.join(cwd, 'python'))
        bases.append(os.path.join(cwd, 'src-tauri', 'python'))
    except Exception:
        pass
    loaded = False
    for base in bases:
        file_path = os.path.join(base, 'faces.py')
        if os.path.exists(file_path):
            spec = importlib.util.spec_from_file_location(module_name, file_path)
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)
            sys.modules[module_name] = mod
            loaded = True
            break
    if not loaded:
        raise ModuleNotFoundError('faces.py not found in candidates: ' + str(bases))
"#,
            p = python_files_path.to_string_lossy()
        );
        py.run(&fallback_import, None, None)
            .map_err(|e| format!("Failed to load faces module: {}", e))?;
        let faces_mod = py.import("faces").map_err(|e| format!("Failed to import faces: {}", e))?;
        let face_cfg = crate::config::get_config().and_then(|c| c.face).unwrap_or_default();
        let det = face_cfg.detection;
        let rec = face_cfg.recognition;
        // 基于当前图像尺寸与可选比例，换算 min/max face size（像素）
        let (min_size_px, max_size_px) = {
            let short_edge = image.width.min(image.height).max(1);
            let min_px = det
                .min_face_ratio
                .and_then(|r| if r > 0.0 { Some(((short_edge as f32) * r).round() as i32) } else { None })
                .unwrap_or(det.min_face_size.unwrap_or(64));
            let max_px = det
                .max_face_ratio
                .and_then(|r| if r > 0.0 { Some(((short_edge as f32) * r).round() as i32) } else { None })
                .unwrap_or(det.max_face_size.unwrap_or(800));
            (min_px, max_px)
        };

        let res: Vec<(i32, i32, i32, i32)> = faces_mod
            .call_method1(
                "detect_targets_or_all_faces",
                (
                    PyBytes::new(py, &image.data),
                    image.width,
                    image.height,
                    det.use_gray,
                    det.image_scale,
                    min_size_px,
                    max_size_px,
                    det.scale_factor,
                    det.min_neighbors,
                    det.confidence_threshold,
                    rec.threshold,
                ),
            )
            .map_err(|e| format!("Failed to call detect_targets_or_all_faces: {}", e))?
            .extract()
            .map_err(|e| format!("Failed to extract faces result: {}", e))?;
        Ok(res.into_iter().map(|(x,y,w,h)| Rect::new(x,y,w,h)).collect())
    })
}

// 检测与识别完全委托给 Python 端
pub fn initialize_face_recognition() -> Result<(), String> {
    if !python_env::is_python_ready() {
        return Err("Python environment is not ready".to_string());
    }
    Python::with_gil(|py| {
        let python_files_path = python_env::get_python_files_path()
            .map_err(|e| format!("Failed to get python files path: {}", e))?;
        let path_setup = format!(
            r#"
import sys, os
sys.path.insert(0, r'{}')
"#,
            python_files_path.to_string_lossy()
        );
        py.run(&path_setup, None, None)
            .map_err(|e| format!("Failed to setup Python path: {}", e))?;

        let load_from_candidates = format!(
            r#"
import sys, os, importlib.util
module_name = 'faces'
# 每次启动都按路径优先级加载 faces.py，避免命名冲突并确保最新
bases = []
try:
    exe_dir = os.path.dirname(sys.executable)
    bases.append(os.path.join(exe_dir, 'python'))
    bases.append(os.path.join(exe_dir, 'src-tauri', 'python'))
except Exception:
    pass
try:
    cwd = os.getcwd()
    bases.append(os.path.join(cwd, 'python'))
    bases.append(os.path.join(cwd, 'src-tauri', 'python'))
except Exception:
    pass
# 最后再考虑 APPDATA 提取目录
bases.append(r'{p}')
loaded = False
for base in bases:
    file_path = os.path.join(base, 'faces.py')
    if os.path.exists(file_path):
        spec = importlib.util.spec_from_file_location(module_name, file_path)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        sys.modules[module_name] = mod
        loaded = True
        break
if not loaded:
    raise ModuleNotFoundError('faces.py not found in candidates: ' + str(bases))
"#,
            p = python_files_path.to_string_lossy()
        );
        py.run(&load_from_candidates, None, None)
            .map_err(|e| format!("Failed to load faces module: {}", e))?;

        let faces = py.import("faces").map_err(|e| format!("Failed to import faces: {}", e))?;
        // 读取配置中的 provider（cpu/cuda/dml），默认 cpu
        let provider = crate::config::get_config()
            .and_then(|c| c.face)
            .map(|f| f.recognition.provider.unwrap_or_else(|| "cpu".to_string()))
            .unwrap_or_else(|| "cpu".to_string());
        let ok: bool = faces
            .call_method1("init_model", (provider.as_str(),))
            .map_err(|e| format!("Failed to call init_model: {}", e))?
            .extract()
            .map_err(|e| format!("Failed to extract init_model result: {}", e))?;
        if !ok { return Err("init_model returned false".to_string()); }
        // 标记模型就绪
        face_model_flag().store(true, Ordering::SeqCst);
        Ok(())
    })
}

pub fn preload_targets_from_faces_dir(_app_handle: &tauri::AppHandle) -> Result<(), String> {
    // 交给 Python 侧 faces.py 进行加载与均值特征的计算（带离群点配置）
    Python::with_gil(|py| {
        let python_files_path = python_env::get_python_files_path()
            .map_err(|e| format!("Failed to get python files path: {}", e))?;
        let path_setup = format!(
            r#"
import sys, os
sys.path.insert(0, r'{}')
"#,
            python_files_path.to_string_lossy()
        );
        py.run(&path_setup, None, None)
            .map_err(|e| format!("Failed to setup Python path: {}", e))?;
        // 与其他入口一致，加入兜底按路径加载 faces.py
        let fallback_import = format!(
            r#"
import sys, os, importlib.util
module_name = 'faces'
try:
    import faces as mod
    _ok = hasattr(mod, 'preload_targets_from_faces_dir') or hasattr(mod, 'init_model')
    if not _ok:
        raise ImportError('conflicting faces module without required attributes')
except Exception:
    bases = []
    bases.append(r'{p}')
    try:
        exe_dir = os.path.dirname(sys.executable)
        bases.append(os.path.join(exe_dir, 'python'))
        bases.append(os.path.join(exe_dir, 'src-tauri', 'python'))
    except Exception:
        pass
    try:
        cwd = os.getcwd()
        bases.append(os.path.join(cwd, 'python'))
        bases.append(os.path.join(cwd, 'src-tauri', 'python'))
    except Exception:
        pass
    loaded = False
    for base in bases:
        file_path = os.path.join(base, 'faces.py')
        if os.path.exists(file_path):
            spec = importlib.util.spec_from_file_location(module_name, file_path)
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)
            sys.modules[module_name] = mod
            loaded = True
            break
    if not loaded:
        raise ModuleNotFoundError('faces.py not found in candidates: ' + str(bases))
"#,
            p = python_files_path.to_string_lossy()
        );
        py.run(&fallback_import, None, None)
            .map_err(|e| format!("Failed to load faces module: {}", e))?;
        let faces = py.import("faces").map_err(|e| format!("Failed to import faces: {}", e))?;
        let rec = crate::config::get_config().and_then(|c| c.face).map(|f| f.recognition).unwrap_or_default();
        let stats: std::collections::HashMap<String, i32> = faces
            .call_method1(
                "preload_targets_from_faces_dir",
                (rec.outlier_threshold.unwrap_or(0.3), rec.outlier_iter.unwrap_or(2)),
            )
            .map_err(|e| format!("Failed to call preload_targets_from_faces_dir: {}", e))?
            .extract()
            .map_err(|e| format!("Failed to extract preload result: {}", e))?;
        info!("[preload_targets] loaded {:?}", stats);
        Ok(())
    })
}
// Rust 不再实现本地 embedding 与匹配，全部交给 Python


