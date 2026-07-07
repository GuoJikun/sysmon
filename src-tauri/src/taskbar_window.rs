use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri::async_runtime::spawn;
use tokio::time::interval;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GetWindowRect, MoveWindow, SetParent, SetWindowPos,
    HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW, WS_EX_NOACTIVATE, GWL_EXSTYLE,
    GetWindowLongW, SetWindowLongW,
};
use windows::core::PCWSTR;
use windows::Win32::UI::HiDpi::GetDpiForWindow;

/// 嵌入模式状态
static EMBED_MODE: OnceLock<Mutex<bool>> = OnceLock::new();
/// MSTaskSwWClass 的原始矩形（嵌入前保存，退出时恢复）
static ORIGINAL_MIN_RECT: OnceLock<Mutex<Option<RECT>>> = OnceLock::new();

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn create_taskbar_window(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    EMBED_MODE.get_or_init(|| Mutex::new(false));
    ORIGINAL_MIN_RECT.get_or_init(|| Mutex::new(None));

    let taskbar_win = WebviewWindowBuilder::new(
        app,
        "taskbar",
        WebviewUrl::App("taskbar.html".into()),
    )
    .title("SysMon Taskbar")
    .inner_size(120.0, 30.0)
    .decorations(false)
    .skip_taskbar(true)
    .resizable(false)
    .visible(false) // 先隐藏，定位后再显示
    .build()?;

    // 获取 HWND
    let hwnd_value = taskbar_win.hwnd()?.0 as isize;

    // 尝试模式 A：SetParent 嵌入到 ReBarWindow32
    let embedded = try_embed_to_taskbar(hwnd_value);

    if !embedded {
        // 模式 B：贴边置顶小窗口 fallback
        taskbar_win.set_always_on_top(true)?;
        taskbar_win.set_decorations(false)?;

        // 透明背景需要通过 Win32 API 设置（Tauri transparent 有时不够）
        let hwnd = HWND(hwnd_value as *mut _);
        unsafe {
            // 设置 WS_EX_NOACTIVATE：不抢焦点
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
            let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_NOACTIVATE.0 as i32);
        }

        position_on_taskbar_fallback(hwnd_value)?;
    }

    taskbar_win.show()?;
    Ok(())
}

/// 尝试模式 A：SetParent 嵌入到任务栏的 ReBarWindow32
fn try_embed_to_taskbar(hwnd_value: isize) -> bool {
    unsafe {
        // 1. 找 Shell_TrayWnd
        let h_taskbar = match FindWindowW(PCWSTR::from_raw(w("Shell_TrayWnd").as_ptr()), None) {
            Ok(h) => h,
            Err(_) => return false,
        };
        if h_taskbar.is_invalid() {
            return false;
        }

        // 2. 找 ReBarWindow32
        let h_bar = match FindWindowExW(
            Some(h_taskbar),
            None,
            PCWSTR::from_raw(w("ReBarWindow32").as_ptr()),
            None,
        ) {
            Ok(h) => h,
            Err(_) => return false,
        };
        if h_bar.is_invalid() {
            return false;
        }

        // 3. 找 MSTaskSwWClass
        let h_min = match FindWindowExW(
            Some(h_bar),
            None,
            PCWSTR::from_raw(w("MSTaskSwWClass").as_ptr()),
            None,
        ) {
            Ok(h) => h,
            Err(_) => return false,
        };
        if h_min.is_invalid() {
            return false;
        }

        // 4. 保存 MSTaskSwWClass 的原始矩形（退出时恢复）
        let mut min_rect = RECT::default();
        if GetWindowRect(h_min, &mut min_rect).is_err() {
            return false;
        }
        *ORIGINAL_MIN_RECT
            .get()
            .expect("ORIGINAL_MIN_RECT")
            .lock()
            .unwrap() = Some(min_rect);

        // 5. 获取 ReBarWindow32 的矩形
        let mut bar_rect = RECT::default();
        if GetWindowRect(h_bar, &mut bar_rect).is_err() {
            return false;
        }
        let bar_width = bar_rect.right - bar_rect.left;
        let bar_height = bar_rect.bottom - bar_rect.top;

        // 6. 获取 DPI 缩放因子
        let hwnd = HWND(hwnd_value as *mut _);
        let dpi = GetDpiForWindow(hwnd);
        let scale = dpi as f32 / 96.0;
        let our_physical_width = (120.0 * scale) as i32;

        // 7. 缩小 MSTaskSwWClass，为我们的窗口留出空间（右侧）
        let _ = MoveWindow(
            h_min,
            0,
            0,
            bar_width - our_physical_width,
            bar_height,
            true,
        );

        // 8. SetParent 将我们的窗口嵌入到 ReBarWindow32
        //    SetParent 返回前一个父窗口；新窗口无父窗口时返回 Err，但 SetParent 仍成功
        let _ = SetParent(hwnd, Some(h_bar));

        // 9. 设置 WS_EX_NOACTIVATE
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_NOACTIVATE.0 as i32);

        // 10. 定位小窗口到 ReBarWindow32 右侧预留空间
        let our_x = bar_width - our_physical_width;
        let our_height = (30.0 * scale) as i32;
        let _ = MoveWindow(hwnd, our_x, 0, our_physical_width, our_height, true);

        *EMBED_MODE.get().expect("EMBED_MODE").lock().unwrap() = true;
        true
    }
}

/// 模式 B：贴边置顶定位到任务栏旁边
fn position_on_taskbar_fallback(hwnd_value: isize) -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let hwnd = HWND(hwnd_value as *mut _);

        let h_taskbar = FindWindowW(PCWSTR::from_raw(w("Shell_TrayWnd").as_ptr()), None)?;
        if h_taskbar.is_invalid() {
            return Err("Cannot find Shell_TrayWnd".into());
        }

        let mut taskbar_rect = RECT::default();
        GetWindowRect(h_taskbar, &mut taskbar_rect)?;

        // 获取 DPI 缩放因子
        let dpi = GetDpiForWindow(hwnd);
        let scale = dpi as f32 / 96.0;
        let physical_width = (120.0 * scale) as i32;
        let physical_height = (30.0 * scale) as i32;

        // 默认假设任务栏在底部（最常见）
        let x = taskbar_rect.right - physical_width;
        let y = taskbar_rect.top;

        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            physical_width,
            physical_height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )?;
    }

    Ok(())
}

/// 后台定时器：每 5s 检查任务栏位置变化并重新定位
pub fn start_taskbar_reposition_timer(app_handle: tauri::AppHandle) {
    spawn(async move {
        let mut tick = interval(Duration::from_secs(5));
        loop {
            tick.tick().await;
            if let Some(win) = app_handle.get_webview_window("taskbar") {
                let hwnd_value = win.hwnd().unwrap_or_default().0 as isize;
                if hwnd_value != 0 {
                    let is_embedded = *EMBED_MODE.get().expect("EMBED_MODE").lock().unwrap();
                    if is_embedded {
                        // 模式 A：重新 AdjustWindowPos（后续完善）
                    } else {
                        // 模式 B：重新贴边定位
                        let _ = position_on_taskbar_fallback(hwnd_value);
                    }
                }
            }
        }
    });
}

/// 退出时回滚：恢复 MSTaskSwWClass 原始尺寸
pub fn cleanup_taskbar_window(hwnd_value: isize) {
    let is_embedded = *EMBED_MODE.get().expect("EMBED_MODE").lock().unwrap();
    if !is_embedded {
        return;
    }
    unsafe {
        let hwnd = HWND(hwnd_value as *mut _);

        // 恢复 SetParent
        let _ = SetParent(hwnd, None); // 恢复为桌面级窗口

        // 恢复 MSTaskSwWClass 原始尺寸
        let h_taskbar = match FindWindowW(PCWSTR::from_raw(w("Shell_TrayWnd").as_ptr()), None) {
            Ok(h) => h,
            Err(_) => return,
        };
        let h_bar = match FindWindowExW(
            Some(h_taskbar),
            None,
            PCWSTR::from_raw(w("ReBarWindow32").as_ptr()),
            None,
        ) {
            Ok(h) => h,
            Err(_) => return,
        };
        let h_min = match FindWindowExW(
            Some(h_bar),
            None,
            PCWSTR::from_raw(w("MSTaskSwWClass").as_ptr()),
            None,
        ) {
            Ok(h) => h,
            Err(_) => return,
        };

        if let Some(orig) = *ORIGINAL_MIN_RECT
            .get()
            .expect("ORIGINAL_MIN_RECT")
            .lock()
            .unwrap()
        {
            let _ = MoveWindow(
                h_min,
                orig.left,
                orig.top,
                orig.right - orig.left,
                orig.bottom - orig.top,
                true,
            );
        }
    }
}
