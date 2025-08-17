use log::{error, info};
use tauri::Manager;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

mod tray;
mod autostart;
mod panic_handler;
mod app_builder;
mod app_state;
pub use app_state::AppState;

use crate::utils::logger;
use crate::config;

const LOG_LEVEL: &str = "debug";

pub fn run() {
    // 自动切换到 exe 所在目录, 为了解决windows自动启动时workding directory读取不到配置文件的问题
    if !cfg!(debug_assertions) {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let _ = std::env::set_current_dir(exe_dir);
            }
        }
    }
    // Initialize config first
    let cfg = config::init_config();

    // Initialize logger
    let log_level = cfg.system.as_ref().and_then(|s| s.log_level.clone()).unwrap_or_else(|| LOG_LEVEL.to_string());
    let _ = logger::init_logger(log_level);
    
    // Initialize COM
    unsafe {
        let result = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if result.is_err() {
            error!("COM initialize failed: {:?}", result.message());
        } else {
            info!("COM initialized (APARTMENTTHREADED)");
        }
    }

    // Initialize app
    let mut builder = app_builder::create_app_builder();
    // Setup application
    builder = builder.setup(move |app| {
        info!("=== application started ===");
        info!("debug mode: {}", cfg!(debug_assertions));

        let app_handle = app.handle();

        // Setup system tray
        tray::setup_tray(&app_handle).expect("Failed to setup system tray");

        // Setup main window
        let main_window = app_handle.get_webview_window("main").unwrap();

        // 设置全局App实例
        let app = AppState {
            handle: app_handle.clone(),
            main_window: main_window.clone(),
        };
        AppState::set_global(app).expect("Failed to set global app instance");
        info!("[✓] global app instance set");

        // Initialize panic handler
        panic_handler::setup_panic_handler(app_handle.clone());
        info!("[✓] panic handler initialized");

        // set autostart
        autostart::set_auto_start(&app_handle).expect("Failed to setup auto start");
        info!("[✓] auto start setup");

		// Initialize Python environment (run in background to avoid blocking UI)
		let app_handle_clone = app_handle.clone();
		tauri::async_runtime::spawn_blocking(move || {
			match crate::ai::python_env::initialize_python_environment_with_app_handle(&app_handle_clone) {
				Ok(()) => info!("[✓] Python environment initialized"),
				Err(e) => {
					error!("[✗] Failed to initialize Python environment: {}", e);
					return;
				}
			}

			// 初始化识别模型并预加载 faces/ 目录的人脸目标向量
			match crate::ai::faces::initialize_face_recognition() {
				Ok(()) => info!("[✓] face recognition model initialized"),
				Err(e) => error!("[✗] face recognition model init failed: {}", e),
			}
			match crate::ai::faces::preload_targets_from_faces_dir(&app_handle_clone) {
				Ok(()) => info!("[✓] preloaded target face embeddings from faces/"),
				Err(e) => error!("[✗] preload target embeddings failed: {}", e),
			}
		});

        info!("=== application initialized ===");
        Ok(())
    });

    // Build and run application
    let app = builder
        .build(tauri::generate_context!("Tauri.toml"))
        .expect("error while building tauri application");

    app.run(|_app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            info!("application is exiting, cleaning up resources...");

            unsafe {
                CoUninitialize();
                info!("[✓] COM uninitialized");
            }
            // 确保监控线程退出
            crate::system::monitoring::stop_monitoring();
        }
    });
}
