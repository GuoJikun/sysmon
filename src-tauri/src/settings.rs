use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub(crate) struct AppSettings {
    pub(crate) taskbar_visible: Option<bool>,
    pub(crate) always_on_top: Option<bool>,
    pub(crate) net_unit: Option<String>,
    pub(crate) log_level: Option<String>,
}

fn settings_file_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_config_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("settings.json"))
}

pub(crate) fn read_settings(app: &tauri::AppHandle) -> AppSettings {
    settings_file_path(app)
        .and_then(|path| std::fs::read_to_string(&path).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub(crate) fn write_settings(app: &tauri::AppHandle, settings: &AppSettings) {
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
///
/// 使用 Win32 ShowWindow 直接控制窗口显隐，
/// 因为 Tauri 的 win.show()/win.hide() 对 SetParent 嵌入的子窗口可能不生效。
///
/// 注意：显示操作需要等待嵌入完成后才执行，否则只保存设置，
/// 由 `embed_taskbar_window` 命令在嵌入成功后自动显示。
#[tauri::command]
pub fn set_taskbar_visible(app: tauri::AppHandle, visible: bool) {
    let mut settings = read_settings(&app);
    settings.taskbar_visible = Some(visible);
    write_settings(&app, &settings);

    if let Some(win) = app.get_webview_window("taskbar") {
        if let Ok(hwnd) = win.hwnd() {
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOWNA};
                if visible {
                    // 只有在已嵌入到任务栏后才显示，否则由定时器在嵌入成功后显示
                    if super::taskbar_window::is_embedded() {
                        let _ = ShowWindow(hwnd, SW_SHOWNA);
                        super::taskbar_window::reposition(hwnd);
                    } else {
                        log::info!("[taskbar] set_taskbar_visible: not yet embedded, timer will show later");
                    }
                } else {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
        }
    }
}

/// 启动时应用任务栏显示设置
pub fn apply_taskbar_setting(app: &tauri::AppHandle) {
    if !read_settings(app).taskbar_visible.unwrap_or(false) {
        if let Some(win) = app.get_webview_window("taskbar") {
            if let Ok(hwnd) = win.hwnd() {
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
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

/// 读取流量单位设置（运行时使用，无需 Tauri 命令）
pub fn get_net_unit_runtime(app: &tauri::AppHandle) -> String {
    read_settings(app)
        .net_unit
        .unwrap_or_else(|| "auto".to_string())
}

/// 获取流量单位设置
#[tauri::command]
pub fn get_net_unit(app: tauri::AppHandle) -> String {
    get_net_unit_runtime(&app)
}

/// 设置流量单位
#[tauri::command]
pub fn set_net_unit(app: tauri::AppHandle, unit: String) {
    let mut settings = read_settings(&app);
    settings.net_unit = Some(unit);
    write_settings(&app, &settings);
}

/// 获取日志等级（返回字符串："off"|"error"|"warn"|"info"|"debug"|"trace"）
#[tauri::command]
pub fn get_log_level(app: tauri::AppHandle) -> String {
    read_settings(&app)
        .log_level
        .unwrap_or_else(|| "info".to_string())
}

/// 设置日志等级（立即生效，同时持久化）
#[tauri::command]
pub fn set_log_level(app: tauri::AppHandle, level: String) {
    let mut settings = read_settings(&app);
    settings.log_level = Some(level.clone());
    write_settings(&app, &settings);

    // 立即生效：更新 log crate 的全局最大等级
    let filter = super::logger::level_from_str(&level);
    log::set_max_level(filter);
    log::info!("Log level changed to {:?}", filter);
}

