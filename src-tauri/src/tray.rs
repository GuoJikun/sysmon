use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, image::Image,
};

/// 创建一个简单的蓝色方块作为图标（32x32 RGBA）
fn create_icon() -> Image<'static> {
    let size = 32u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    // 画一个蓝色圆角方块
    for y in 0..size {
        for x in 0..size {
            // 简单圆角：角落像素设为透明
            let dx = if x < 4 { 4 - x } else if x >= size - 4 { x - (size - 5) } else { 0 };
            let dy = if y < 4 { 4 - y } else if y >= size - 4 { y - (size - 5) } else { 0 };
            let alpha = if dx * dx + dy * dy > 9 { 0 } else { 255 };
            rgba.extend_from_slice(&[74, 144, 217, alpha]); // #4A90D9
        }
    }
    Image::new_owned(rgba, size, size)
}

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let _tray = TrayIconBuilder::new()
        .icon(create_icon())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(win) = app.get_webview_window("main") {
                    win.show().ok();
                    win.set_focus().ok();
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
        })
        .on_tray_icon_event(|tray, event| {
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
        })
        .tooltip("SysMon - 系统监控")
        .build(app)?;

    Ok(())
}
