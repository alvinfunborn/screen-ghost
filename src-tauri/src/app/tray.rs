use log::info;
use tauri::{image::Image, menu::{MenuBuilder, MenuItemBuilder}, tray::{TrayIconBuilder, TrayIconEvent}, AppHandle, Manager};

const SHOW_TRAY_ICON: bool = true;

pub fn setup_tray(
    app_handle: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    if !SHOW_TRAY_ICON {
        info!("[setup_tray] tray icon is not enabled");
        return Ok(());
    }

    let exit_item = MenuItemBuilder::with_id("exit", "Exit").build(app_handle)?;
    let restart_item = MenuItemBuilder::with_id("restart", "Restart").build(app_handle)?;
    let settings_item = MenuItemBuilder::with_id("settings", "Settings").build(app_handle)?;

    let tray_menu = MenuBuilder::new(app_handle)
        .item(&settings_item)
        .item(&restart_item)
        .item(&exit_item)
        .build()?;

    let tray_icon = Image::from_bytes(include_bytes!("../../icons/icon.ico"))?;

    let _tray_icon = TrayIconBuilder::new()
        .menu(&tray_menu)
        .on_menu_event(move |tray_handle, event| {
            let app_handle = tray_handle.app_handle();
            match event.id.as_ref() {
                "exit" => {
                    app_handle.exit(0);
                }
                "settings" => {
                    let window = app_handle.get_webview_window("main").unwrap();
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
                "restart" => {
                    app_handle.restart();
                }
                _ => {}
            }
        })
        .icon(tray_icon)
        .on_tray_icon_event(move |tray_handle, event| {
            let app_handle = tray_handle.app_handle();
            match event {
                TrayIconEvent::DoubleClick { .. } => {
                    let window = app_handle.get_webview_window("main").unwrap();
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
                _ => {}
            }
        })
        .show_menu_on_left_click(true)
        .build(app_handle)?;
    Ok(())
}
