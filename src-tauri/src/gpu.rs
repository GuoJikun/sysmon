use std::sync::{Mutex, OnceLock};

use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW, PdhRemoveCounter, PDH_FMT_DOUBLE, PDH_FMT_COUNTERVALUE_ITEM_W,
    PDH_HCOUNTER, PDH_HQUERY,
};
use windows::core::PCWSTR;

#[repr(transparent)]
struct SafePdhQuery(PDH_HQUERY);
unsafe impl Send for SafePdhQuery {}
unsafe impl Sync for SafePdhQuery {}

#[repr(transparent)]
struct SafePdhCounter(PDH_HCOUNTER);
unsafe impl Send for SafePdhCounter {}
unsafe impl Sync for SafePdhCounter {}

pub struct GpuMonitor {
    query: SafePdhQuery,
    counter: SafePdhCounter,
}

static GPU_MONITOR: OnceLock<Mutex<Option<GpuMonitor>>> = OnceLock::new();

pub fn init_gpu_monitor() -> Result<(), String> {
    let monitor_slot = GPU_MONITOR.get_or_init(|| Mutex::new(None));
    let mut guard = monitor_slot.lock().unwrap();

    unsafe {
        let mut query = PDH_HQUERY::default();
        let status = PdhOpenQueryW(None, 0, &mut query);
        if status != 0 {
            return Err(format!("PdhOpenQueryW failed with status: {}", status));
        }

        let mut counter = PDH_HCOUNTER::default();
        // GPU Engine(*)\Utilization Percentage — 通配符路径获取所有 GPU 引擎实例
        let counter_path = w("\\GPU Engine(*)\\Utilization Percentage");
        let status = PdhAddEnglishCounterW(query, PCWSTR::from_raw(counter_path.as_ptr()), 0, &mut counter);
        if status != 0 {
            // 如果通配符方式失败，回退方案在后续实现
            PdhCloseQuery(query);
            return Err(format!(
                "PdhAddEnglishCounterW failed with status: {}. GPU monitoring unavailable.",
                status
            ));
        }

        // PDH 需两次 CollectData 才返回真实值，预 Collect 一次
        PdhCollectQueryData(query);

        *guard = Some(GpuMonitor {
            query: SafePdhQuery(query),
            counter: SafePdhCounter(counter),
        });
    }

    Ok(())
}

pub fn get_gpu_usage() -> f32 {
    let monitor_slot = GPU_MONITOR.get_or_init(|| Mutex::new(None));
    let mut guard = monitor_slot.lock().unwrap();

    let monitor = match guard.as_mut() {
        Some(m) => m,
        None => return 0.0, // GPU 监控未初始化
    };

    unsafe {
        PdhCollectQueryData(monitor.query.0);

        let mut buffer_size: u32 = 0;
        let mut item_count: u32 = 0;

        // 第一次调用获取所需缓冲区大小
        let status = PdhGetFormattedCounterArrayW(
            monitor.counter.0,
            PDH_FMT_DOUBLE,
            &mut buffer_size as *mut u32,
            &mut item_count as *mut u32,
            None,
        );

        // PDH_MORE_DATA (0x800007D2) 是预期返回值
        if status != 0 && status != 0x800007D2u32 {
            // 如果不是 PDH_MORE_DATA，尝试处理
            if buffer_size == 0 {
                return 0.0;
            }
        }

        if buffer_size == 0 || item_count == 0 {
            return 0.0;
        }

        // 分配缓冲区
        let buffer = vec![0u8; buffer_size as usize];
        let items_ptr = buffer.as_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;

        let status = PdhGetFormattedCounterArrayW(
            monitor.counter.0,
            PDH_FMT_DOUBLE,
            &mut buffer_size as *mut u32,
            &mut item_count as *mut u32,
            Some(items_ptr),
        );

        if status != 0 {
            return 0.0;
        }

        // 遍历所有实例，过滤 engtype_3D 求和
        let items = std::slice::from_raw_parts(items_ptr, item_count as usize);
        let mut total_3d_usage: f64 = 0.0;

        for item in items {
            let name = &item.szName;
            let name_str = name.to_string().unwrap_or_default();
            // 实例名格式: pid_XXXX_luid_..._eng_X_engtype_3D
            // 过滤含 engtype_3D 的实例
            if name_str.contains("engtype_3D") {
                // PDH_FMT_DOUBLE 结果在 item.Anonymous.doubleValue
                let value = item.FmtValue.Anonymous.doubleValue;
                total_3d_usage += value;
            }
        }

        // Cap 到 100%
        total_3d_usage.min(100.0) as f32
    }
}

pub fn cleanup_gpu_monitor() {
    let monitor_slot = GPU_MONITOR.get();
    if let Some(monitor_slot) = monitor_slot {
        let mut guard = monitor_slot.lock().unwrap();
        if let Some(m) = guard.take() {
            unsafe {
                PdhRemoveCounter(m.counter.0);
                PdhCloseQuery(m.query.0);
            }
        }
    }
}

/// Helper: 将字符串转为 UTF-16 广字符数组（含 null terminator）
fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
