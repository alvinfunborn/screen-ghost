use std::process::{Command, Stdio};
use std::path::{Path, PathBuf};
use std::fs;
// removed unused io imports
use std::env;
use log::{info, warn, error};
use once_cell::sync::OnceCell;
use tauri::Emitter;

use crate::api::emitter;


static PYTHON_ENV_MANAGER: OnceCell<PythonEnvManager> = OnceCell::new();

#[derive(Debug)]
pub struct PythonEnvManager {
    python_path: Option<PathBuf>,
    virtual_env_path: Option<PathBuf>,
    is_initialized: bool,
    app_handle: Option<tauri::AppHandle>,
}

impl PythonEnvManager {
    pub fn new() -> Self {
        Self {
            python_path: None,
            virtual_env_path: None,
            is_initialized: false,
            app_handle: None,
        }
    }

    pub fn set_app_handle(&mut self, app_handle: tauri::AppHandle) {
        self.app_handle = Some(app_handle);
    }

    pub fn get_instance() -> &'static PythonEnvManager {
        PYTHON_ENV_MANAGER.get_or_init(|| {
            let mut manager = PythonEnvManager::new();
            if let Err(e) = manager.initialize() {
                error!("Failed to initialize Python environment manager: {}", e);
            }
            manager
        })
    }

    pub fn initialize(&mut self) -> Result<(), String> {
        if self.is_initialized {
            return Ok(());
        }

        info!("Initializing Python environment manager...");
        emitter::emit_toast("正在初始化 Python 环境…");
    
        // 1. 提取Python文件到临时目录
        emitter::emit_toast("正在提取 Python 资源文件…");
        let python_files_path = self.extract_python_files()?;
        info!("Python files extracted to: {:?}", python_files_path);

        // 2. 检测系统Python
        emitter::emit_toast("正在检测系统 Python…");
        if let Some(python_path) = self.detect_system_python()? {
            self.python_path = Some(python_path.clone());
            info!("Found system Python at: {:?}", python_path);
            
            // 3. 检查系统Python是否满足要求
            if self.check_system_python_requirements(&python_path)? {
                info!("System Python meets requirements, using system Python");
                emitter::emit_toast("系统 Python 就绪，正在完成初始化…");
                self.is_initialized = true;
                emitter::emit_toast_close();
                return Ok(());
            } else {
                info!("System Python found but missing required packages");
                
                // 尝试在系统Python中安装缺失的包
                info!("Attempting to install missing packages in system Python...");
                emitter::emit_toast("系统 Python 缺少依赖，正在安装缺失包…");
                if self.install_packages_in_system_python(&python_path)? {
                    info!("Successfully installed packages in system Python");
                    emitter::emit_toast("依赖安装完成，正在完成初始化…");
                    self.is_initialized = true;
                    emitter::emit_toast_close();
                    return Ok(());
                } else {
                    info!("Failed to install packages in system Python, falling back to virtual environment");
                    emitter::emit_toast("系统 Python 依赖安装失败，回退到虚拟环境…");
                }
            }
        } else {
            info!("No system Python found");
            emitter::emit_toast("未检测到系统 Python，尝试使用本地/虚拟环境…");

            // Windows 平台尝试本地静默安装到 APPDATA
            #[cfg(target_os = "windows")]
            {
                match self.find_or_install_local_python_on_windows() {
                    Ok(Some(local_python)) => {
                        info!("Installed/Found local Python at: {:?}", local_python);
                        self.python_path = Some(local_python);
                    }
                    Ok(None) => {
                        info!("Local Python not found and installation skipped");
                    }
                    Err(e) => {
                        warn!("Install local Python failed: {}", e);
                    }
                }
            }
        }

        // 4. 如果系统/本地Python不可用，创建虚拟环境（需要先确保有可用的python可执行文件）
        info!("Creating virtual environment as fallback...");
        emitter::emit_toast("正在创建虚拟环境…");
        let virtual_env_path = self.create_virtual_environment()?;
        self.virtual_env_path = Some(virtual_env_path.clone());
        info!("Created virtual environment at: {:?}", virtual_env_path);

        // 5. 安装必要的包
        emitter::emit_toast("正在安装必要依赖…");
        self.install_required_packages(&virtual_env_path)?;

        // 6. 最终验证
        emitter::emit_toast("正在验证环境…");
        if !self.verify_environment_ready()? {
            emitter::emit_toast("Python 环境验证失败");
            return Err("Python environment verification failed after installation".to_string());
        }

        self.is_initialized = true;
        info!("Python environment manager initialized successfully");
        emitter::emit_toast("Python 环境初始化完成");
        emitter::emit_toast_close();
        Ok(())
    }

    fn detect_system_python(&self) -> Result<Option<PathBuf>, String> {
        let python_commands = ["python", "python3", "python3.11", "python3.10", "python3.9", "python3.8"];
        
        for cmd in &python_commands {
            if let Ok(output) = Command::new(cmd)
                .arg("--version")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
            {
                if output.status.success() {
                    // 获取Python可执行文件的完整路径
                    if let Ok(output) = Command::new(cmd)
                        .arg("-c")
                        .arg("import sys; print(sys.executable)")
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output()
                    {
                        if output.status.success() {
                            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                            return Ok(Some(PathBuf::from(path)));
                        }
                    }
                }
            }
        }
        
        Ok(None)
    }

    #[cfg(target_os = "windows")]
    fn get_local_python_install_dir(&self) -> Result<PathBuf, String> {
        let app_dir = self.get_app_data_dir()?;
        Ok(app_dir.join("python311"))
    }

    #[cfg(target_os = "windows")]
    fn find_installed_python_in_local_dir(&self) -> Option<PathBuf> {
        if let Ok(dir) = self.get_local_python_install_dir() {
            let exe = dir.join("python.exe");
            if exe.exists() {
                return Some(exe);
            }
        }
        None
    }

    #[cfg(target_os = "windows")]
    fn find_or_install_local_python_on_windows(&self) -> Result<Option<PathBuf>, String> {
        if let Some(path) = self.find_installed_python_in_local_dir() {
            return Ok(Some(path));
        }

        let target_dir = self.get_local_python_install_dir()?;
        if !target_dir.exists() {
            fs::create_dir_all(&target_dir).map_err(|e| format!("Create target dir failed: {}", e))?;
        }

        // 下载并静默安装官方 Python 3.11 x64 到用户目录
        let temp_dir = std::env::temp_dir();
        let installer_path = temp_dir.join("python-3.11.9-amd64.exe");

        if !installer_path.exists() {
            let url = "https://www.python.org/ftp/python/3.11.9/python-3.11.9-amd64.exe";
            info!("Downloading Python installer from: {}", url);

            // 使用 PowerShell 下载，避免引入额外依赖
            let download = Command::new("powershell")
                .arg("-NoProfile")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-Command")
                .arg(format!(
                    "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri '{}' -OutFile '{}'",
                    url,
                    installer_path.display()
                ))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();

            match download {
                Ok(out) if out.status.success() => info!("Python installer downloaded to: {:?}", installer_path),
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr);
                    return Err(format!("Download installer failed: {}", err));
                }
                Err(e) => return Err(format!("Execute PowerShell failed: {}", e)),
            }
        }

        // 运行静默安装
        info!("Installing Python silently to {:?}", target_dir);
        let status = Command::new(&installer_path)
            .arg("/quiet")
            .arg("InstallAllUsers=0")
            .arg("PrependPath=0")
            .arg("Include_pip=1")
            .arg(format!("TargetDir={}", target_dir.display()))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .status()
            .map_err(|e| format!("Failed to start installer: {}", e))?;

        if !status.success() {
            return Err("Python installer exited with non-zero status".to_string());
        }

        // 校验安装结果
        if let Some(exe) = self.find_installed_python_in_local_dir() {
            Ok(Some(exe))
        } else {
            Err("Python not found after installation".to_string())
        }
    }

    fn check_system_python_requirements(&self, python_path: &Path) -> Result<bool, String> {
        let required_packages = ["cv2", "numpy"];
        
        for package in &required_packages {
            let result = Command::new(python_path)
                .arg("-c")
                .arg(&format!("import {}", package))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();
            
            if result.is_err() || !result.unwrap().status.success() {
                warn!("Required package '{}' not found in system Python", package);
                return Ok(false);
            }
        }
        
        Ok(true)
    }

    fn create_virtual_environment(&self) -> Result<PathBuf, String> {
        let app_data_dir = self.get_app_data_dir()?;
        let venv_path = app_data_dir.join("python_env");
        
        // 如果虚拟环境已存在，直接返回
        if venv_path.exists() {
            info!("Virtual environment already exists at: {:?}", venv_path);
            return Ok(venv_path);
        }

        // 创建虚拟环境
        let python_path = self.python_path.as_ref()
            .ok_or("No Python executable found")?;
        
        let result = Command::new(python_path)
            .arg("-m")
            .arg("venv")
            .arg(&venv_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        
        match result {
            Ok(output) if output.status.success() => {
                info!("Created virtual environment at: {:?}", venv_path);
                Ok(venv_path)
            }
            Ok(output) => {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                Err(format!("Failed to create virtual environment: {}", error_msg))
            }
            Err(e) => Err(format!("Failed to execute venv command: {}", e))
        }
    }



    fn install_required_packages(&self, venv_path: &Path) -> Result<(), String> {
        let pip_path = self.get_pip_path(venv_path)?;
        let required_packages = ["opencv-python", "numpy"];
        let app_handle = self.app_handle.clone();
        
        // 发送开始安装事件
        if let Some(ref handle) = app_handle {
            let _ = handle.emit("python-installation-started", "开始安装Python包...");
        }
        
        // 同步安装包
        for (index, package) in required_packages.iter().enumerate() {
            info!("Installing package: {}", package);
            
            // 发送进度更新
            if let Some(ref handle) = app_handle {
                let progress = (index as f64 / required_packages.len() as f64) * 100.0;
                let _ = handle.emit("python-installation-progress", format!(
                    "正在安装 {}... ({:.1}%)", package, progress
                ));
            }
            
            // 尝试安装包
            let result = Command::new(&pip_path)
                .arg("install")
                .arg(package)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();
            
            match result {
                Ok(output) if output.status.success() => {
                    info!("Successfully installed {}", package);
                    
                    // 发送成功消息
                    if let Some(ref handle) = app_handle {
                        let _ = handle.emit("python-installation-success", format!(
                            "成功安装 {}", package
                        ));
                    }
                }
                Ok(output) => {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    warn!("Failed to install {}: {}", package, error_msg);
                    
                    // 尝试使用--user标志安装
                    let result = Command::new(&pip_path)
                        .arg("install")
                        .arg("--user")
                        .arg(package)
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output();
                    
                    match result {
                        Ok(output) if output.status.success() => {
                            info!("Successfully installed {} with --user flag", package);
                            
                            // 发送成功消息
                            if let Some(ref handle) = app_handle {
                                let _ = handle.emit("python-installation-success", format!(
                                    "成功安装 {} (用户模式)", package
                                ));
                            }
                        }
                        Ok(output) => {
                            let error_msg = String::from_utf8_lossy(&output.stderr);
                            error!("Failed to install {}: {}", package, error_msg);
                            
                            // 发送错误消息
                            if let Some(ref handle) = app_handle {
                                let _ = handle.emit("python-installation-error", format!(
                                    "安装失败 {}: {}", package, error_msg
                                ));
                            }
                            
                            return Err(format!("Failed to install {}: {}", package, error_msg));
                        }
                        Err(e) => {
                            error!("Failed to execute pip command for {}: {}", package, e);
                            
                            // 发送错误消息
                            if let Some(ref handle) = app_handle {
                                let _ = handle.emit("python-installation-error", format!(
                                    "执行pip命令失败 {}: {}", package, e
                                ));
                            }
                            
                            return Err(format!("Failed to execute pip command for {}: {}", package, e));
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to execute pip command for {}: {}", package, e);
                    
                    // 发送错误消息
                    if let Some(ref handle) = app_handle {
                        let _ = handle.emit("python-installation-error", format!(
                            "执行pip命令失败 {}: {}", package, e
                        ));
                    }
                    
                    return Err(format!("Failed to execute pip command for {}: {}", package, e));
                }
            }
        }
        
        // 安装完成后，验证环境
        info!("Verifying installed packages...");
        if let Some(ref handle) = app_handle {
            let _ = handle.emit("python-installation-progress", "验证安装的包...");
        }
        
        // 验证包是否正确安装
        if !self.verify_packages_installed(venv_path)? {
            return Err("Package installation verification failed".to_string());
        }
        
        // 发送完成消息
        if let Some(ref handle) = app_handle {
            let _ = handle.emit("python-installation-completed", "Python包安装完成！");
        }
        
        Ok(())
    }

    fn verify_packages_installed(&self, venv_path: &Path) -> Result<bool, String> {
        let python_path = self.get_python_executable_from_venv(venv_path)?;
        let required_packages = ["cv2", "numpy"];
        
        for package in &required_packages {
            let result = Command::new(&python_path)
                .arg("-c")
                .arg(&format!("import {}", package))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();
            
            if result.is_err() || !result.unwrap().status.success() {
                warn!("Package verification failed for '{}'", package);
                return Ok(false);
            }
        }
        
        info!("All required packages verified successfully");
        Ok(true)
    }

    fn get_python_executable_from_venv(&self, venv_path: &Path) -> Result<PathBuf, String> {
        #[cfg(target_os = "windows")]
        let python_name = "python.exe";
        #[cfg(not(target_os = "windows"))]
        let python_name = "python";
        
        let python_path = venv_path.join("Scripts").join(python_name);
        if python_path.exists() {
            return Ok(python_path);
        }
        
        let python_path = venv_path.join("bin").join(python_name);
        if python_path.exists() {
            return Ok(python_path);
        }
        
        Err("Could not find python executable in virtual environment".to_string())
    }

    fn install_packages_in_system_python(&self, python_path: &Path) -> Result<bool, String> {
        let required_packages = ["opencv-python", "numpy"];
        let app_handle = self.app_handle.clone();
        
        // 发送开始安装事件
        if let Some(ref handle) = app_handle {
            let _ = handle.emit("python-installation-started", "在系统Python中安装包...");
        }
        
        for (index, package) in required_packages.iter().enumerate() {
            info!("Installing package in system Python: {}", package);
            
            // 发送进度更新
            if let Some(ref handle) = app_handle {
                let progress = (index as f64 / required_packages.len() as f64) * 100.0;
                let _ = handle.emit("python-installation-progress", format!(
                    "正在安装 {}... ({:.1}%)", package, progress
                ));
            }
            
            // 尝试安装包
            let result = Command::new(python_path)
                .arg("-m")
                .arg("pip")
                .arg("install")
                .arg(package)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();
            
            match result {
                Ok(output) if output.status.success() => {
                    info!("Successfully installed {} in system Python", package);
                    
                    // 发送成功消息
                    if let Some(ref handle) = app_handle {
                        let _ = handle.emit("python-installation-success", format!(
                            "成功安装 {}", package
                        ));
                    }
                }
                Ok(output) => {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    warn!("Failed to install {} in system Python: {}", package, error_msg);
                    
                    // 尝试使用--user标志安装
                    let result = Command::new(python_path)
                        .arg("-m")
                        .arg("pip")
                        .arg("install")
                        .arg("--user")
                        .arg(package)
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output();
                    
                    match result {
                        Ok(output) if output.status.success() => {
                            info!("Successfully installed {} with --user flag in system Python", package);
                            
                            // 发送成功消息
                            if let Some(ref handle) = app_handle {
                                let _ = handle.emit("python-installation-success", format!(
                                    "成功安装 {} (用户模式)", package
                                ));
                            }
                        }
                        Ok(output) => {
                            let error_msg = String::from_utf8_lossy(&output.stderr);
                            error!("Failed to install {} in system Python: {}", package, error_msg);
                            
                            // 发送错误消息
                            if let Some(ref handle) = app_handle {
                                let _ = handle.emit("python-installation-error", format!(
                                    "安装失败 {}: {}", package, error_msg
                                ));
                            }
                            
                            return Ok(false);
                        }
                        Err(e) => {
                            error!("Failed to execute pip command for {}: {}", package, e);
                            
                            // 发送错误消息
                            if let Some(ref handle) = app_handle {
                                let _ = handle.emit("python-installation-error", format!(
                                    "执行pip命令失败 {}: {}", package, e
                                ));
                            }
                            
                            return Ok(false);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to execute pip command for {}: {}", package, e);
                    
                    // 发送错误消息
                    if let Some(ref handle) = app_handle {
                        let _ = handle.emit("python-installation-error", format!(
                            "执行pip命令失败 {}: {}", package, e
                        ));
                    }
                    
                    return Ok(false);
                }
            }
        }
        
        // 验证安装
        if self.check_system_python_requirements(python_path)? {
            info!("System Python packages verified successfully");
            
            // 发送完成消息
            if let Some(ref handle) = app_handle {
                let _ = handle.emit("python-installation-completed", "系统Python包安装完成！");
            }
            
            Ok(true)
        } else {
            warn!("System Python packages verification failed after installation");
            Ok(false)
        }
    }

    fn verify_environment_ready(&self) -> Result<bool, String> {
        // 检查系统Python
        if let Some(ref python_path) = self.python_path {
            if self.check_system_python_requirements(python_path)? {
                return Ok(true);
            }
        }
        
        // 检查虚拟环境
        if let Some(ref venv_path) = self.virtual_env_path {
            if self.verify_packages_installed(venv_path)? {
                return Ok(true);
            }
        }
        
        Ok(false)
    }

    fn get_pip_path(&self, venv_path: &Path) -> Result<PathBuf, String> {
        #[cfg(target_os = "windows")]
        let pip_name = "pip.exe";
        #[cfg(not(target_os = "windows"))]
        let pip_name = "pip";
        
        let pip_path = venv_path.join("Scripts").join(pip_name);
        if pip_path.exists() {
            return Ok(pip_path);
        }
        
        let pip_path = venv_path.join("bin").join(pip_name);
        if pip_path.exists() {
            return Ok(pip_path);
        }
        
        Err("Could not find pip executable in virtual environment".to_string())
    }

    #[cfg(target_os = "windows")]
    fn append_python_dir_to_process_env(&self) {
        if let Some(ref python) = self.python_path {
            if let Some(dir) = python.parent() {
                let scripts = dir.join("Scripts");
                let old_path = env::var("PATH").unwrap_or_default();
                let mut new_path = format!("{};{}", dir.display(), old_path);
                if scripts.exists() {
                    new_path = format!("{};{}", scripts.display(), new_path);
                }
                env::set_var("PATH", new_path);
                env::set_var("PYTHONHOME", dir);
            }
        }
    }

    fn extract_python_files(&self) -> Result<PathBuf, String> {
        let app_data_dir = self.get_app_data_dir()?;
        let python_files_dir = app_data_dir.join("python_files");
        
        // 确保目标目录存在
        if !python_files_dir.exists() {
            fs::create_dir_all(&python_files_dir)
                .map_err(|e| format!("Failed to create python files directory: {}", e))?;
        }
        
        // 开发环境：每次启动都覆盖拷贝，确保新增/更新的脚本可用（例如新增的 face_recognition.py）
        #[cfg(debug_assertions)]
        {
            let src_python_dir = PathBuf::from("src-tauri/python");
            if src_python_dir.exists() {
                self.copy_dir_all(&src_python_dir, &python_files_dir)
                    .map_err(|e| format!("Failed to copy python files: {}", e))?;
                // 调试：输出目录内容，确认脚本同步
                if let Ok(read_dir) = fs::read_dir(&python_files_dir) {
                    let mut names: Vec<String> = Vec::new();
                    for entry in read_dir.flatten() {
                        names.push(entry.file_name().to_string_lossy().to_string());
                    }
                    info!("python_files content: {:?}", names);
                }
                return Ok(python_files_dir);
            }
        }
        
        // 生产环境：若此前已存在则直接使用；否则尝试从源码目录复制（作为兜底）
        #[cfg(not(debug_assertions))]
        {
            if python_files_dir.exists() {
                return Ok(python_files_dir);
            }
            let src_python_dir = PathBuf::from("src-tauri/python");
            if src_python_dir.exists() {
                self.copy_dir_all(&src_python_dir, &python_files_dir)
                    .map_err(|e| format!("Failed to copy python files: {}", e))?;
                if let Ok(read_dir) = fs::read_dir(&python_files_dir) {
                    let mut names: Vec<String> = Vec::new();
                    for entry in read_dir.flatten() {
                        names.push(entry.file_name().to_string_lossy().to_string());
                    }
                    info!("python_files content: {:?}", names);
                }
                return Ok(python_files_dir);
            }
        }
        
        Err("Could not find or extract python files".to_string())
    }
    
    fn copy_dir_all(&self, src: &Path, dst: &Path) -> Result<(), String> {
        if !dst.exists() {
            fs::create_dir(dst)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
        
        for entry in fs::read_dir(src)
            .map_err(|e| format!("Failed to read directory: {}", e))?
        {
            let entry = entry
                .map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let ty = entry.file_type()
                .map_err(|e| format!("Failed to get file type: {}", e))?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            
            if ty.is_dir() {
                self.copy_dir_all(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)
                    .map_err(|e| format!("Failed to copy file: {}", e))?;
            }
        }
        
        Ok(())
    }

    fn get_app_data_dir(&self) -> Result<PathBuf, String> {
        #[cfg(target_os = "windows")]
        {
            let app_data = std::env::var("APPDATA")
                .map_err(|_| "Could not get APPDATA environment variable".to_string())?;
            Ok(PathBuf::from(app_data).join("screen-ghost-rust"))
        }
        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME")
                .map_err(|_| "Could not get HOME environment variable".to_string())?;
            Ok(PathBuf::from(home).join("Library/Application Support/screen-ghost-rust"))
        }
        #[cfg(target_os = "linux")]
        {
            let home = std::env::var("HOME")
                .map_err(|_| "Could not get HOME environment variable".to_string())?;
            Ok(PathBuf::from(home).join(".config/screen-ghost-rust"))
        }
    }

    // 移除未使用的 get_python_executable（对外提供全局函数即可）

    pub fn prepare_process_env(&self) {
        #[cfg(target_os = "windows")]
        {
            self.append_python_dir_to_process_env();
        }
    }

    pub fn is_ready(&self) -> bool {
        self.is_initialized
    }

    pub fn get_installation_guide(&self) -> String {
        r#"
Python环境安装指南：

1. 安装Python 3.7或更高版本：
   - Windows: 从 https://www.python.org/downloads/ 下载安装
   - macOS: 使用 brew install python3
   - Linux: 使用包管理器安装 python3

2. 安装必要的Python包：
   pip install opencv-python numpy

3. 如果遇到权限问题，请使用：
   pip install --user opencv-python numpy

4. 重启应用程序

如果问题仍然存在，请联系技术支持。
        "#.to_string()
    }

    pub fn get_python_files_path(&self) -> Result<PathBuf, String> {
        let app_data_dir = self.get_app_data_dir()?;
        let python_files_dir = app_data_dir.join("python_files");
        
        if python_files_dir.exists() {
            Ok(python_files_dir)
        } else {
            Err("Python files not found. Please ensure the application is properly installed.".to_string())
        }
    }
}

pub fn initialize_python_environment() -> Result<(), String> {
    PythonEnvManager::get_instance();
    Ok(())
}

pub fn initialize_python_environment_with_app_handle(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let mut manager = PythonEnvManager::new();
    manager.set_app_handle(app_handle.clone());
    manager.initialize()?;
    // 将带有 app_handle 的已初始化管理器注册为全局单例，
    // 确保后续 get_instance()/is_python_ready() 使用同一实例
    let _ = PYTHON_ENV_MANAGER.set(manager);
    Ok(())
}

// 移除未使用的对外 get_python_executable 包装

pub fn is_python_ready() -> bool {
    PythonEnvManager::get_instance().is_ready()
}

pub fn get_installation_guide() -> String {
    PythonEnvManager::get_instance().get_installation_guide()
}

pub fn get_python_files_path() -> Result<PathBuf, String> {
    PythonEnvManager::get_instance().get_python_files_path()
} 