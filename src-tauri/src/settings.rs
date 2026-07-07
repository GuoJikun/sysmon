use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

#[derive(Serialize, Deserialize, Default)]
struct AppSettings {
    taskbar_visible: Option<bool>,
    always_on_top: Option<bool>,
}

fn settings_file_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_config_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("settings.json"))
}

fn read_settings(app: &tauri::AppHandle) -> AppSettings {
    settings_file_path(app)
        .and_then(|path| std::fs::read_to_string(&path).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_settings(app: &tauri::AppHandle, settings: &AppSettings) {
    if let (Some(path), Ok(json)) = (settings_file_path(app), serde_json::to_string_pretty(settings))
    {
        let _ = std::fs::write(&path, json);
    }
}

/// 获取任务栏窗口显示状态
#[tauri::command]
pub fn get_taskbar_visible(app: tauri::AppHandle) -> bool {
    read_settings(&app).taskbar_visible.unwrap_or(false)
}

/// 设置任务栏窗口显示状态
#[tauri::command]
pub fn set_taskbar_visible(app: tauri::AppHandle, visible: bool) {
    let mut settings = read_settings(&app);
    settings.taskbar_visible = Some(visible);
    write_settings(&app, &settings);

    if let Some(win) = app.get_webview_window("taskbar") {
        if visible {
            let _ = win.show();
        } else {
            let _ = win.hide();
        }
    }
}

/// 启动时应用任务栏显示设置
pub fn apply_taskbar_setting(app: &tauri::AppHandle) {
    if !read_settings(app).taskbar_visible.unwrap_or(false) {
        if let Some(win) = app.get_webview_window("taskbar") {
            let _ = win.hide();
        }
    }
}

/// 获取主窗口置顶状态
#[tauri::command]
pub fn get_always_on_top(app: tauri::AppHandle) -> bool {
    read_settings(&app).always_on_top.unwrap_or(true)
}

/// 设置主窗口置顶状态
#[tauri::command]
pub fn set_always_on_top(app: tauri::AppHandle, enabled: bool) {
    let mut settings = read_settings(&app);
    settings.always_on_top = Some(enabled);
    write_settings(&app, &settings);

    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_always_on_top(enabled);
    }
}

/// 启动时应用置顶设置
pub fn apply_always_on_top_setting(app: &tauri::AppHandle) {
    let enabled = read_settings(app).always_on_top.unwrap_or(true);
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_always_on_top(enabled);
    }
}
