mod commands;
mod gpu;
mod settings;
mod sys_info;
mod taskbar_window;
mod tray;

use std::time::Duration;

use tauri::Emitter;
use tauri::async_runtime::spawn;
use tokio::time::interval;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            settings::get_taskbar_visible,
            settings::set_taskbar_visible,
            settings::get_always_on_top,
            settings::set_always_on_top
        ])
        .setup(|app| {
            // 1. 初始化系统信息采集器
            sys_info::init();

            // 2. 初始化 GPU 监控（PDH）
            if let Err(e) = gpu::init_gpu_monitor() {
                eprintln!("GPU monitor init failed: {}", e);
            }

            // 3. 创建系统托盘
            tray::setup_tray(app)?;

            // 4. 创建任务栏内嵌小窗口
            taskbar_window::create_taskbar_window(app)?;

            // 5. 应用任务栏显示设置（用户可能在设置中关闭了）
            settings::apply_taskbar_setting(app.handle());

            // 6. 应用窗口置顶设置
            settings::apply_always_on_top_setting(app.handle());

            // 7. 启动后台数据采集 + 推送定时器
            start_data_push_timer(app.handle().clone());

            // 8. 启动任务栏重定位定时器
            taskbar_window::start_taskbar_reposition_timer(app.handle().clone());

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
                    }
                    // settings 窗口允许正常关闭
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

            if let Ok(info) = sys_info::get_current_info(gpu_usage) {
                let _ = app_handle.emit_to("main", "sys-info", info);
            }

            if let Ok(net) = sys_info::get_net_speed_info() {
                let _ = app_handle.emit_to("taskbar", "net-speed", net);
            }
        }
    });
}
