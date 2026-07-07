//! 任务栏内嵌窗口模块
//!
//! 借鉴 SkyDesk2 方案：将窗口 SetParent 到 Shell_TrayWnd（任务栏主窗口），
//! 然后根据 TrayNotifyWnd 的位置持续重定位。
//! 兼容 Windows 10 / 11（Shell_TrayWnd、TrayNotifyWnd 在全版本均存在）。
//!
//! 关键设计：
//! - SetParent 必须在前端页面加载完成后由 JS 通过 Tauri command 触发
//!   （此时 WebView2 已完全初始化，SetParent 才能生效）
//! - Rust setup 阶段只创建窗口，不执行嵌入
//! - 嵌入成功后启动后台定时器持续重定位

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri::async_runtime::spawn;
use tokio::time::interval;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::{ABM_GETTASKBARPOS, APPBARDATA, SHAppBarMessage};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GetParent, GetWindowRect, MoveWindow, SetParent,
    SetWindowPos, ShowWindow, SW_SHOWNA, SWP_HIDEWINDOW, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
};

/// 任务栏窗口逻辑宽度（DPI 缩放前，单位 px @96dpi）
const LOGICAL_WIDTH: f64 = 120.0;
/// 任务栏窗口逻辑高度（DPI 缩放前，单位 px @96dpi）
const LOGICAL_HEIGHT: f64 = 28.0;

/// 是否已成功嵌入（用于防止定时器重复启动）
static EMBEDDED: AtomicBool = AtomicBool::new(false);

/// 返回任务栏窗口是否已成功嵌入
pub fn is_embedded() -> bool {
    EMBEDDED.load(Ordering::Relaxed)
}

/// 将 &str 转为 UTF-16 空终止字符串
fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 创建任务栏窗口（仅创建，不执行嵌入）
///
/// 嵌入操作由前端页面加载完成后通过 `embed_taskbar_window` 命令触发。
pub fn create_taskbar_window(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("[taskbar] creating taskbar window");

    let _taskbar_win = WebviewWindowBuilder::new(
        app,
        "taskbar",
        WebviewUrl::App("taskbar.html".into()),
    )
    .title("SysMon Taskbar")
    .inner_size(LOGICAL_WIDTH, LOGICAL_HEIGHT)
    .decorations(false)
    .skip_taskbar(true)
    .resizable(false)
    .transparent(true)
    .shadow(false)
    .build()?;

    log::info!("[taskbar] window created, waiting for frontend to invoke embed command");
    Ok(())
}

/// Tauri 命令：将任务栏窗口嵌入到 Shell_TrayWnd
///
/// 由 taskbar.js 在页面加载完成后调用（`invoke('embed_taskbar_window')`）。
/// 此时 WebView2 已完全初始化，SetParent 能够正确生效。
#[tauri::command]
pub fn embed_taskbar_window(app: tauri::AppHandle) {
    if EMBEDDED.load(Ordering::Relaxed) {
        log::info!("[taskbar] embed: already embedded, skipping");
        return;
    }

    let Some(win) = app.get_webview_window("taskbar") else {
        log::error!("[taskbar] embed: taskbar window not found");
        return;
    };

    let Ok(hwnd) = win.hwnd() else {
        log::error!("[taskbar] embed: failed to get HWND");
        return;
    };

    unsafe {
        // 查找 Shell_TrayWnd
        let h_taskbar = match FindWindowW(PCWSTR::from_raw(w("Shell_TrayWnd").as_ptr()), None) {
            Ok(h) if !h.is_invalid() => h,
            _ => {
                log::error!("[taskbar] embed: Shell_TrayWnd not found!");
                return;
            }
        };

        // 根据设置决定是否显示（在 SetParent 之前读取）
        let should_show = super::settings::read_settings(&app)
            .taskbar_visible
            .unwrap_or(false);

        // 在 SetParent 之前隐藏窗口（SetParent 后窗口变成子窗口，ShowWindow 可能不生效）
        if !should_show {
            // 使用 SetWindowPos + SWP_HIDEWINDOW（比 ShowWindow(SW_HIDE) 更可靠）
            let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0, 
                SWP_HIDEWINDOW | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER);
            log::info!("[taskbar] embed: hiding window BEFORE SetParent (taskbar_visible=false)");
        }

        // 执行 SetParent（SkyDesk2 方案：直接调用，不改窗口样式）
        let result = SetParent(hwnd, Some(h_taskbar));
        log::info!("[taskbar] embed: SetParent returned {:?}", result);

        // 验证嵌入是否成功：检查 GetParent 是否返回 Shell_TrayWnd
        if let Ok(parent) = GetParent(hwnd) {
            if parent == h_taskbar {
                log::info!("[taskbar] embed: verified - parent is Shell_TrayWnd");
            } else {
                log::error!(
                    "[taskbar] embed: WARNING - parent mismatch! expected {:?}, got {:?}",
                    h_taskbar, parent
                );
            }
        } else {
            log::error!("[taskbar] embed: GetParent failed");
        }

        EMBEDDED.store(true, Ordering::Relaxed);
        log::info!("[taskbar] embed: SUCCESS");

        // SetParent 之后，根据需要显示或隐藏窗口
        if should_show {
            let _ = ShowWindow(hwnd, SW_SHOWNA);
            log::info!("[taskbar] embed: window shown (taskbar_visible=true)");
        } else {
            // SetParent 后窗口可能再次可见，需要再次隐藏
            let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0, 
                SWP_HIDEWINDOW | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER);
            log::info!("[taskbar] embed: window hidden AFTER SetParent (taskbar_visible=false)");
        }
    }

    // 始终启动重定位定时器（内部会检查 taskbar_visible，隐藏时跳过）
    // 这样用户后续在设置中开启时，定时器已在运行，能持续定位
    start_taskbar_reposition_timer(app);
}

/// 将窗口定位到 TrayNotifyWnd 左侧（横向任务栏）或上方（纵向任务栏），居中对齐。
pub fn reposition(hwnd: HWND) {
    unsafe {
        let h_taskbar = match FindWindowW(PCWSTR::from_raw(w("Shell_TrayWnd").as_ptr()), None) {
            Ok(h) if !h.is_invalid() => h,
            _ => return,
        };

        let dpi = GetDpiForWindow(hwnd);
        let scale = if dpi > 0 { dpi as f64 / 96.0 } else { 1.0 };
        let win_width = (LOGICAL_WIDTH * scale) as i32;
        let win_height = (LOGICAL_HEIGHT * scale) as i32;

        // 获取任务栏屏幕矩形，用于判断方向
        let mut abd = APPBARDATA::default();
        abd.cbSize = std::mem::size_of::<APPBARDATA>() as u32;
        if SHAppBarMessage(ABM_GETTASKBARPOS, &mut abd) == 0 {
            return;
        }
        let is_horizontal = (abd.rc.right - abd.rc.left) > (abd.rc.bottom - abd.rc.top);

        // 将 TrayNotifyWnd 的屏幕坐标转为 Shell_TrayWnd 客户区坐标
        let h_tray = FindWindowExW(
            Some(h_taskbar), None,
            PCWSTR::from_raw(w("TrayNotifyWnd").as_ptr()), None,
        )
        .ok()
        .filter(|h| !h.is_invalid());

        let (tray_x, tray_y, tray_h) = match h_tray {
            Some(h) => {
                let mut r = RECT::default();
                if GetWindowRect(h, &mut r).is_ok() {
                    let mut lt = POINT { x: r.left, y: r.top };
                    let mut rb = POINT { x: r.right, y: r.bottom };
                    let _ = ScreenToClient(h_taskbar, &mut lt);
                    let _ = ScreenToClient(h_taskbar, &mut rb);
                    (lt.x, lt.y, rb.y - lt.y)
                } else {
                    // fallback: 用任务栏右边缘
                    let mut pt = POINT { x: abd.rc.right, y: abd.rc.top };
                    let _ = ScreenToClient(h_taskbar, &mut pt);
                    (pt.x, pt.y, abd.rc.bottom - abd.rc.top)
                }
            }
            None => {
                let mut pt = POINT { x: abd.rc.right, y: abd.rc.top };
                let _ = ScreenToClient(h_taskbar, &mut pt);
                (pt.x, pt.y, abd.rc.bottom - abd.rc.top)
            }
        };

        // 横向：紧贴 TrayNotifyWnd 左侧，垂直居中
        // 纵向：紧贴 TrayNotifyWnd 上方，水平居中（用任务栏宽度居中）
        let (x, y) = if is_horizontal {
            (tray_x - win_width, tray_y + (tray_h - win_height) / 2)
        } else {
            let tb_w_client = {
                let mut pt = POINT { x: abd.rc.right, y: abd.rc.bottom };
                let _ = ScreenToClient(h_taskbar, &mut pt);
                pt.x
            };
            (tb_w_client / 2 - win_width / 2, tray_y - win_height)
        };

        log::info!(
            "[taskbar] reposition: pos({}, {}), size {}x{}, tray=({},{} {}x{})",
            x, y, win_width, win_height, tray_x, tray_y, 0, tray_h
        );
        let _ = MoveWindow(hwnd, x, y, win_width, win_height, false);
    }
}

/// 后台定时器：持续重定位任务栏窗口
///
/// 每 500ms 重新计算位置，适配任务栏自动隐藏、位置变更、DPI 变化等场景。
fn start_taskbar_reposition_timer(app_handle: tauri::AppHandle) {
    spawn(async move {
        let mut tick = interval(Duration::from_millis(500));
        let mut log_counter = 0;
        loop {
            tick.tick().await;
            let taskbar_visible = super::settings::read_settings(&app_handle)
                .taskbar_visible
                .unwrap_or(false);
            // 每 10 秒记录一次定时器状态（便于调试）
            if log_counter % 20 == 0 {
                log::info!("[taskbar] timer tick: taskbar_visible={}", taskbar_visible);
            }
            log_counter += 1;
            // 仅在任务栏窗口需要显示时才重定位
            if !taskbar_visible {
                continue;
            }
            if let Some(win) = app_handle.get_webview_window("taskbar") {
                if let Ok(hwnd) = win.hwnd() {
                    reposition(hwnd);
                }
            }
        }
    });
}

/// 退出时将窗口从任务栏分离
pub fn cleanup_taskbar_window(hwnd_value: isize) {
    unsafe {
        let hwnd = HWND(hwnd_value as *mut _);
        let _ = SetParent(hwnd, None);
        EMBEDDED.store(false, Ordering::Relaxed);
        log::info!("[taskbar] cleanup: window detached from taskbar");
    }
}
