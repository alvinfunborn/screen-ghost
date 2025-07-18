use log::{error, info};
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

pub mod dispatcher;
mod tray;
mod autostart;
mod panic_handler;
mod app_builder;

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

    // // Initialize logger
    // let _ = init_logger(config.system.logging_level.clone());
    
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
