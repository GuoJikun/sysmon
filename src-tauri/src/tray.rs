use tauri::{
    menu::Menu,
    tray::{MouseButton, MouseButtonState, TrayIconEvent},
    Manager,
};

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let tray = app
        .tray_by_id("main")
        .ok_or("Tray icon 'main' not found")?;

    // 设置空菜单（不显示任何项），右键事件由 on_tray_icon_event 处理
    let empty_menu = Menu::with_items(app, &[])?;
    tray.set_menu(Some(empty_menu))?;
    tray.set_tooltip(Some("SysMon - 系统监控"))?;
    tray.set_show_menu_on_left_click(false)?;

    tray.on_tray_icon_event(|tray, event| {
        let app = tray.app_handle();

        match event {
            // 左键：显示主窗口
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                crate::tray_popup_window::hide_tray_popup(app);
                if let Some(win) = app.get_webview_window("main") {
                    win.show().ok();
                    win.set_focus().ok();
                }
            }
            // 右键：弹出自定义窗口
            TrayIconEvent::Click {
                button: MouseButton::Right,
                button_state: MouseButtonState::Up,
                position,
                rect,
                ..
            } => {
                // 从 rect 枚举中提取托盘图标的屏幕边界（物理像素）
                let (icon_x, icon_y, icon_w) = match (rect.position, rect.size) {
                    (
                        tauri::Position::Physical(pos),
                        tauri::Size::Physical(size),
                    ) => (pos.x as f64, pos.y as f64, size.width as f64),
                    _ => (position.x, position.y, 0.0),
                };

                crate::tray_popup_window::show_tray_popup(
                    app, icon_x, icon_y, icon_w, position.x, position.y,
                );
            }
            _ => {}
        }
    });

    Ok(())
}
