use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const WINDOW_LABEL: &str = "tray-popup";
const POPUP_WIDTH: f64 = 200.0;
const POPUP_HEIGHT: f64 = 152.0;

static VISIBLE: AtomicBool = AtomicBool::new(false);
/// 弹窗刚显示时的保护标志：阻止 blur 事件立即隐藏弹窗。
/// 首次创建弹窗时 WebView2 初始化会导致主窗口失焦，触发 blur→hide 竞态，
/// 通过此标志在 500ms 内忽略 blur 触发的隐藏请求。
static SUPPRESS_BLUR_HIDE: AtomicBool = AtomicBool::new(false);

/// 为弹窗应用圆角区域和系统阴影（Win32 API）
#[cfg(target_os = "windows")]
fn apply_popup_effects(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, GetWindowRect, SetWindowLongPtrW, GWL_STYLE,
    };

    let Ok(hwnd) = window.hwnd() else {
        return;
    };

    // 1. 圆角区域（与主窗口一致：16×16 椭圆 → 10px 视觉圆角）
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok() {
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        let region = unsafe { CreateRoundRectRgn(0, 0, width + 1, height + 1, 16, 16) };
        if !region.is_invalid() {
            unsafe { SetWindowRgn(hwnd, Some(region), true) };
        }
    }

    // 2. 系统阴影（CS_DROPSHADOW）
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
    unsafe { SetWindowLongPtrW(hwnd, GWL_STYLE, style | 0x00020000) }; // CS_DROPSHADOW
}

/// 获取已有弹窗窗口，或首次创建（初始隐藏）
fn get_or_create(app: &tauri::AppHandle) -> tauri::WebviewWindow {
    if let Some(win) = app.get_webview_window(WINDOW_LABEL) {
        return win;
    }

    let win = WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::App("tray-popup.html".into()))
        .inner_size(POPUP_WIDTH, POPUP_HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .visible(false)
        .always_on_top(true)
        .build()
        .expect("failed to create tray popup window");

    // 预创建时就应用圆角 + 阴影，后续 show 时无需重复设置
    #[cfg(target_os = "windows")]
    apply_popup_effects(&win);

    win
}

/// 在 setup 阶段预创建弹窗窗口（隐藏），让 WebView2 提前完成初始化。
/// 避免用户首次右键主窗口时因 WebView2 异步初始化导致的显示失败。
pub fn precreate(app: &tauri::AppHandle) {
    get_or_create(app);
    log::info!("Tray popup window pre-created (hidden)");
}

/// 在托盘图标附近显示弹窗；若已显示则隐藏（toggle 效果）
///
/// - `icon_x/icon_y/icon_w`：托盘图标的屏幕边界矩形（物理像素）
/// - `cursor_x/cursor_y`：鼠标光标位置（作为回退定位）
pub fn show_tray_popup(
    app: &tauri::AppHandle,
    icon_x: f64,
    icon_y: f64,
    icon_w: f64,
    cursor_x: f64,
    cursor_y: f64,
) {
    let win = get_or_create(app);
    log::info!("[popup] show_tray_popup: got window, label={}", WINDOW_LABEL);

    // 已可见 → 隐藏（再次右键托盘图标时 toggle）
    if VISIBLE.load(Ordering::SeqCst) {
        log::info!("[popup] already visible, hiding (toggle)");
        win.hide().ok();
        VISIBLE.store(false, Ordering::SeqCst);
        return;
    }

    // 计算弹窗位置：居中于托盘图标上方
    let (target_x, target_y) = if icon_w > 0.0 {
        (
            icon_x + (icon_w - POPUP_WIDTH) / 2.0,
            icon_y - POPUP_HEIGHT - 4.0,
        )
    } else {
        // 回退：使用光标位置
        (
            cursor_x - POPUP_WIDTH / 2.0,
            cursor_y - POPUP_HEIGHT - 4.0,
        )
    };

    // 如果弹窗超出屏幕顶部，改为在光标下方显示
    let final_y = if target_y < 0.0 {
        cursor_y + 4.0
    } else {
        target_y
    };

    win.set_position(tauri::PhysicalPosition::new(target_x as i32, final_y as i32))
        .ok();
    log::info!("[popup] set position to ({}, {}), calling show()", target_x, final_y);
    win.show().ok();
    log::info!("[popup] show() called, setting focus");

    win.set_focus().ok();
    VISIBLE.store(true, Ordering::SeqCst);
    log::info!("[popup] set_focus done, VISIBLE=true, SUPPRESS_BLUR_HIDE=true for 500ms");

    // 启动 blur 保护期：首次创建弹窗时 WebView2 初始化会导致主窗口失焦，
    // 触发 blur→hide 竞态。500ms 内忽略 blur 触发的隐藏请求。
    SUPPRESS_BLUR_HIDE.store(true, Ordering::SeqCst);
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        SUPPRESS_BLUR_HIDE.store(false, Ordering::SeqCst);
        // 保护期结束后，如果弹窗仍然可见，确保它拥有焦点
        if VISIBLE.load(Ordering::SeqCst) {
            if let Some(win) = app_clone.get_webview_window(WINDOW_LABEL) {
                let _ = win.set_focus();
            }
        }
    });

    // 通知弹窗前端刷新对号状态（直接携带状态，避免异步闪烁）
    let main_visible = app
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    let taskbar_visible = super::settings::read_settings(app)
        .taskbar_visible
        .unwrap_or(false);
    let _ = app.emit_to(
        WINDOW_LABEL,
        "popup-shown",
        serde_json::json!({
            "main_visible": main_visible,
            "taskbar_visible": taskbar_visible
        }),
    );
}

/// 隐藏弹窗
pub fn hide_tray_popup(app: &tauri::AppHandle) {
    if !VISIBLE.load(Ordering::SeqCst) {
        return;
    }
    if let Some(win) = app.get_webview_window(WINDOW_LABEL) {
        win.hide().ok();
    }
    VISIBLE.store(false, Ordering::SeqCst);
}

/// 前端 blur 检测后调用此命令隐藏弹窗
#[tauri::command]
pub fn hide_tray_popup_cmd(app: tauri::AppHandle) {
    let suppress = SUPPRESS_BLUR_HIDE.load(Ordering::SeqCst);
    log::info!("[popup] hide_tray_popup_cmd called, SUPPRESS_BLUR_HIDE={}", suppress);
    // 保护期内忽略 blur 触发的隐藏请求（防止首次创建 WebView2 时的竞态）
    if suppress {
        log::info!("[popup] hiding suppressed during protection period");
        return;
    }
    hide_tray_popup(&app);
}

/// 主窗口右键调用：在光标位置显示弹窗（x/y 为逻辑屏幕坐标）
#[tauri::command]
pub fn show_popup_at_cursor(app: tauri::AppHandle, x: f64, y: f64) {
    log::info!("[popup] show_popup_at_cursor called: x={}, y={}", x, y);
    let scale = app
        .get_webview_window("main")
        .and_then(|w| w.scale_factor().ok())
        .unwrap_or(1.0);
    let px = x * scale;
    let py = y * scale;
    log::info!("[popup] physical coords: px={}, py={}, scale={}", px, py, scale);
    show_tray_popup(&app, 0.0, 0.0, 0.0, px, py);
}

/// 查询主窗口是否可见
#[tauri::command]
pub fn is_main_visible(app: tauri::AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

/// 切换任务栏显示状态（由托盘弹窗菜单调用）
#[tauri::command]
pub fn toggle_taskbar_visible(app: tauri::AppHandle) {
    // 自动隐藏弹窗
    hide_tray_popup(&app);

    let mut settings = crate::settings::read_settings(&app);
    let new_visible = !settings.taskbar_visible.unwrap_or(false);
    settings.taskbar_visible = Some(new_visible);
    crate::settings::write_settings(&app, &settings);

    // 应用任务栏显隐
    if let Some(win) = app.get_webview_window("taskbar") {
        if let Ok(hwnd) = win.hwnd() {
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOWNA};
                if new_visible {
                    if crate::taskbar_window::is_embedded() {
                        let _ = ShowWindow(hwnd, SW_SHOWNA);
                        crate::taskbar_window::reposition(hwnd);
                    }
                } else {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
        }
    }

    // 通知所有窗口设置已变更
    let _ = app.emit("settings-changed", serde_json::json!({ "taskbar_visible": new_visible }));
}

/// 弹窗内菜单项的操作处理（由前端 invoke 调用）
#[tauri::command]
pub fn tray_popup_action(app: tauri::AppHandle, action: String) {
    // 先隐藏弹窗
    hide_tray_popup(&app);

    match action.as_str() {
        "show" => {
            if let Some(win) = app.get_webview_window("main") {
                let new_visible = if win.is_visible().unwrap_or(false) {
                    win.hide().ok();
                    false
                } else {
                    win.show().ok();
                    win.set_focus().ok();
                    true
                };
                // 通知弹窗更新对号状态（弹窗已隐藏，更新 DOM 避免下次打开闪烁）
                let _ = app.emit_to(WINDOW_LABEL, "main-visibility-changed", new_visible);
            }
        }
        "settings" => {
            if let Some(win) = app.get_webview_window("settings") {
                win.show().ok();
                win.set_focus().ok();
            }
        }
        "quit" => {
            // 清理任务栏窗口
            if let Some(win) = app.get_webview_window("taskbar") {
                let hwnd_value = win.hwnd().unwrap_or_default().0 as isize;
                crate::taskbar_window::cleanup_taskbar_window(hwnd_value);
            }
            // 清理 GPU 监控
            crate::gpu::cleanup_gpu_monitor();
            app.exit(0);
        }
        _ => {}
    }
}
