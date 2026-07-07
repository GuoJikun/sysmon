use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconEvent},
    Manager, WebviewUrl, WebviewWindowBuilder,
};

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_item, &settings_item, &quit_item])?;

    // 获取 Tauri 配置（tauri.conf.json trayIcon）创建的托盘图标
    let tray = app
        .tray_by_id("main")
        .ok_or("Tray icon 'main' not found")?;

    tray.set_menu(Some(menu))?;
    tray.set_tooltip(Some("SysMon - 系统监控"))?;
    tray.set_show_menu_on_left_click(false)?;

    tray.on_menu_event(|app, event| match event.id.as_ref() {
        "show" => {
            if let Some(win) = app.get_webview_window("main") {
                win.show().ok();
                win.set_focus().ok();
            }
        }
        "settings" => {
            if let Some(win) = app.get_webview_window("settings") {
                win.show().ok();
                win.set_focus().ok();
            } else {
                let _ = WebviewWindowBuilder::new(
                    app,
                    "settings",
                    WebviewUrl::App("settings.html".into()),
                )
                .title("设置")
                .inner_size(800.0, 600.0)
                .resizable(false)
                .center()
                .skip_taskbar(true)
                .build();
            }
        }
        "quit" => {
            // 清理任务栏窗口（回滚 MSTaskSwWClass 等）
            if let Some(win) = app.get_webview_window("taskbar") {
                let hwnd_value = win.hwnd().unwrap_or_default().0 as isize;
                crate::taskbar_window::cleanup_taskbar_window(hwnd_value);
            }
            crate::gpu::cleanup_gpu_monitor();
            app.exit(0);
        }
        _ => {}
    });

    tray.on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            let app = tray.app_handle();
            if let Some(win) = app.get_webview_window("main") {
                win.show().ok();
                win.set_focus().ok();
            }
        }
    });

    Ok(())
}
