mod commands;
mod gpu;
mod logger;
mod settings;
mod sys_info;
mod taskbar_window;
mod tray;
mod tray_popup_window;

use std::time::Duration;

use tauri::Emitter;
use tauri::Manager;
use tauri::async_runtime::spawn;
use tokio::time::interval;

#[cfg(target_os = "windows")]
fn set_main_window_rounded_region(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let Ok(hwnd) = window.hwnd() else {
        eprintln!("Failed to get main window HWND");
        return;
    };

    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        eprintln!("Failed to get main window rect");
        return;
    }

    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    // 10px corner radius => 20x20 ellipse
    let region = unsafe { CreateRoundRectRgn(0, 0, width + 1, height + 1, 16, 16) };
    if region.is_invalid() {
        eprintln!("Failed to create rounded window region");
        return;
    }

    unsafe { SetWindowRgn(hwnd, Some(region), true) };
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            settings::get_taskbar_visible,
            settings::set_taskbar_visible,
            settings::get_always_on_top,
            settings::set_always_on_top,
            settings::get_net_unit,
            settings::set_net_unit,
            settings::get_log_level,
            settings::set_log_level,
            taskbar_window::embed_taskbar_window,
            tray_popup_window::tray_popup_action,
            tray_popup_window::hide_tray_popup_cmd,
            tray_popup_window::show_popup_at_cursor,
            tray_popup_window::is_main_visible,
            tray_popup_window::toggle_taskbar_visible,
        ])
        .setup(|app| {
            // 0. 初始化日志（必须在最前，后续代码才能用 log::info! 等宏）
            let log_level = app
                .path()
                .app_config_dir()
                .ok()
                .and_then(|_dir| {
                    settings::read_settings(&app.handle())
                        .log_level
                        .map(|s| logger::level_from_str(&s))
                })
                .unwrap_or(log::LevelFilter::Info);
            if let Some(config_dir) = app.path().app_config_dir().ok() {
                logger::init(log_level, &config_dir);
            }

            // 1. 初始化系统信息采集器
            sys_info::init();

            // 2. 初始化 GPU 监控（PDH）
            if let Err(e) = gpu::init_gpu_monitor() {
                log::warn!("GPU monitor init failed: {}", e);
            }

            // 3. 创建系统托盘
            tray::setup_tray(app)?;

            // 3.5 预创建托盘弹窗窗口（隐藏），让 WebView2 提前完成初始化
            // 避免用户首次右键主窗口时因 WebView2 异步初始化导致的显示失败
            tray_popup_window::precreate(app.handle());

            // 4. 创建任务栏内嵌小窗口
            taskbar_window::create_taskbar_window(app)?;

            // 5. 应用任务栏显示设置（用户可能在设置中关闭了）
            settings::apply_taskbar_setting(app.handle());

            // 6. 应用窗口置顶设置
            settings::apply_always_on_top_setting(app.handle());

            // 7. 主窗口应用圆角区域（Windows 透明无边框窗口需要 Win32 区域裁剪）
            #[cfg(target_os = "windows")]
            if let Some(main_window) = app.get_webview_window("main") {
                set_main_window_rounded_region(&main_window);
            }

            // 8. 启动后台数据采集 + 推送定时器
            start_data_push_timer(app.handle().clone());

            // 9. 任务栏重定位定时器由 embed_taskbar_window 命令在前端加载后启动

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    let label = window.label();
                    if label == "main" {
                        // 主窗口关闭 → 隐藏而非退出（托盘后台运行）
                        api.prevent_close();
                        window.hide().ok();
                    } else if label == "taskbar" {
                        // taskbar 窗口也阻止关闭
                        api.prevent_close();
                    } else if label == "settings" {
                        // settings 窗口关闭 → 隐藏而非销毁（保留 webview 状态）
                        api.prevent_close();
                        window.hide().ok();
                    }
                    // tray-popup 窗口允许正常关闭
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn start_data_push_timer(app_handle: tauri::AppHandle) {
    spawn(async move {
        let mut tick = interval(Duration::from_millis(1500));

        loop {
            tick.tick().await;

            let gpu_usage = gpu::get_gpu_usage();
            let net_unit = settings::get_net_unit_runtime(&app_handle);

            // 网速只计算一次，同时供主窗口和任务栏窗口使用
            let (net_down, net_up) = sys_info::compute_net_speed();

            if let Ok(info) = sys_info::get_current_info(gpu_usage, net_down, net_up, &net_unit) {
                let _ = app_handle.emit_to("main", "sys-info", info);
            }

            let net = sys_info::get_net_speed_info(net_down, net_up, &net_unit);
            let _ = app_handle.emit_to("taskbar", "net-speed", net);
        }
    });
}
