use log::{error, info};
use tauri::{AppHandle, Manager, WindowEvent, WebviewWindow};
use tauri_plugin_autostart::MacosLauncher;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use once_cell::sync::Lazy;
use std::sync::Mutex;

mod tray;
mod autostart;
mod panic_handler;
mod app_builder;
mod app_state;
pub use app_state::AppState;

use crate::{system, utils::logger, ai::{face_detect, face_recognition}};

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
    // // Initialize config first
    // config::init_config();
    // let config = config::get_config().unwrap();
    // let config_for_manage = config.clone();

    // Initialize logger
    let _ = logger::init_logger(LOG_LEVEL.to_string());
    
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

        // // Handle window visibility
        // if config.system.start_in_tray {
        //     if let Err(e) = main_window.hide() {
        //         error!("[✗] hide main window failed: {}", e);
        //     }
        //     info!("[✓] minimized to tray (if show_tray_icon is true)");
        // } else {
        //     if let Err(e) = main_window.show() {
        //         error!("[✗] show main window failed: {}", e);
        //     }
        // }

        // Initialize panic handler
        panic_handler::setup_panic_handler(app_handle.clone());
        info!("[✓] panic handler initialized");

        // // Initialize input hook
        // input::hook::init(app_handle.clone());
        // info!("[✓] input hook initialized");

        // set autostart
        autostart::set_auto_start(&app_handle).expect("Failed to setup auto start");
        info!("[✓] auto start setup");

        // Initialize Python environment for face detection
        if let Err(e) = face_detect::initialize_python_environment_with_app_handle(&app_handle) {
            error!("[✗] Failed to initialize Python environment: {}", e);
        } else {
            info!("[✓] Python environment initialized for face detection");
        }

        // Initialize face recognition and preload targets from exe_dir/faces
        if let Err(e) = face_recognition::initialize_face_recognition() {
            error!("[✗] init face recognition failed: {}", e);
        } else {
            info!("[✓] face recognition model initialized");
            if let Err(e) = face_recognition::preload_targets_from_faces_dir(&app_handle) {
                error!("[✗] preload targets failed: {}", e);
            } else {
                info!("[✓] preload targets done");
            }
        }

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
        }
    });
}
