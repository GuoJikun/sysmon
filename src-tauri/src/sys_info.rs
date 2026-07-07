use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use sysinfo::{CpuRefreshKind, MemoryRefreshKind, Networks, RefreshKind, System};

use crate::commands::{NetSpeedInfo, SystemInfo};

static SYS: OnceLock<Mutex<System>> = OnceLock::new();
static NETWORKS: OnceLock<Mutex<Networks>> = OnceLock::new();
static LAST_REFRESH_TIME: OnceLock<Mutex<Instant>> = OnceLock::new();

/// 上一轮网速采样的累计字节数（用于增量计算）
static PREV_NET_RX: OnceLock<Mutex<u64>> = OnceLock::new();
static PREV_NET_TX: OnceLock<Mutex<u64>> = OnceLock::new();

pub fn init() {
    let mut sys = System::new();
    // 预刷新 CPU（sysinfo 需两次 refresh 间隔才有真实值）
    sys.refresh_cpu_usage();
    SYS.set(Mutex::new(sys)).unwrap();

    let networks = Networks::new_with_refreshed_list();
    NETWORKS.set(Mutex::new(networks)).unwrap();

    // 记录初始累计值
    let (rx, tx) = get_current_net_totals();
    PREV_NET_RX.set(Mutex::new(rx)).unwrap();
    PREV_NET_TX.set(Mutex::new(tx)).unwrap();

    LAST_REFRESH_TIME.set(Mutex::new(Instant::now())).unwrap();
}

fn get_sys() -> &'static Mutex<System> {
    SYS.get().expect("SYS not initialized")
}

fn get_networks() -> &'static Mutex<Networks> {
    NETWORKS.get().expect("NETWORKS not initialized")
}

fn should_skip_interface(name: &str) -> bool {
    name.contains("Loopback")
        || name.starts_with("lo")
        || name.contains("vEthernet")
        || name.contains("Hyper-V")
        || name.contains("veth")
        || name.contains("docker")
        || name.contains("vnic")
}

/// 获取所有非虚拟网卡的累计接收/发送字节数
fn get_current_net_totals() -> (u64, u64) {
    let networks = get_networks().lock().unwrap();
    let mut total_received: u64 = 0;
    let mut total_transmitted: u64 = 0;
    for (name, data) in &*networks {
        if should_skip_interface(name) {
            continue;
        }
        total_received += data.received();
        total_transmitted += data.transmitted();
    }
    (total_received, total_transmitted)
}

/// 根据单位设置格式化网速（完整格式，带 /s 后缀，用于主窗口）
fn format_speed(bytes_per_sec: f64, unit: &str) -> String {
    match unit {
        "kb" => format!("{:.1} KB/s", bytes_per_sec / 1024.0),
        "mb" => format!("{:.2} MB/s", bytes_per_sec / 1_048_576.0),
        _ => {
            // auto
            if bytes_per_sec < 1024.0 {
                format!("{:.0} B/s", bytes_per_sec)
            } else if bytes_per_sec < 1_048_576.0 {
                format!("{:.1} KB/s", bytes_per_sec / 1024.0)
            } else {
                format!("{:.1} MB/s", bytes_per_sec / 1_048_576.0)
            }
        }
    }
}

/// 根据单位设置格式化网速（短格式，无 /s 后缀，用于任务栏窗口）
fn format_speed_short(bytes_per_sec: f64, unit: &str) -> String {
    match unit {
        "kb" => format!("{:.0}K", bytes_per_sec / 1024.0),
        "mb" => format!("{:.1}M", bytes_per_sec / 1_048_576.0),
        _ => {
            // auto
            if bytes_per_sec < 1024.0 {
                format!("{:.0}B", bytes_per_sec)
            } else if bytes_per_sec < 1_048_576.0 {
                format!("{:.0}K", bytes_per_sec / 1024.0)
            } else {
                format!("{:.1}M", bytes_per_sec / 1_048_576.0)
            }
        }
    }
}

pub fn get_current_info(gpu_usage: f32, net_down: f64, net_up: f64, net_unit: &str) -> Result<SystemInfo, String> {
    let mut sys = get_sys().lock().unwrap();
    sys.refresh_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );

    let cpu_avg = {
        let cpus = sys.cpus();
        if cpus.is_empty() {
            0.0
        } else {
            cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32
        }
    };

    let mem_total = sys.total_memory();
    let mem_used = sys.used_memory();
    let mem_pct = if mem_total > 0 {
        (mem_used as f32 / mem_total as f32) * 100.0
    } else {
        0.0
    };

    Ok(SystemInfo {
        cpu: cpu_avg,
        mem_used,
        mem_total,
        mem_pct,
        gpu: gpu_usage,
        net_down,
        net_up,
        net_down_str: format_speed(net_down, net_unit),
        net_up_str: format_speed(net_up, net_unit),
    })
}

pub fn get_net_speed_info(down: f64, up: f64, net_unit: &str) -> NetSpeedInfo {
    NetSpeedInfo {
        down,
        up,
        down_str: format_speed_short(down, net_unit),
        up_str: format_speed_short(up, net_unit),
    }
}

/// 增量计算网速：当前累计 - 上次累计 / 时间间隔
pub fn compute_net_speed() -> (f64, f64) {
    // 1. 刷新网卡数据
    let mut networks = get_networks().lock().unwrap();
    networks.refresh(true);
    drop(networks); // 释放锁后再读取 totals

    // 2. 获取当前累计值
    let (current_rx, current_tx) = get_current_net_totals();

    // 3. 计算时间差
    let now = Instant::now();
    let mut last_time = LAST_REFRESH_TIME
        .get()
        .expect("LAST_REFRESH_TIME")
        .lock()
        .unwrap();
    let elapsed = now.duration_since(*last_time).as_secs_f64();
    *last_time = now;

    if elapsed < 0.001 {
        return (0.0, 0.0); // 防止除零
    }

    // 4. 计算增量
    let mut prev_rx = PREV_NET_RX.get().expect("PREV_NET_RX").lock().unwrap();
    let mut prev_tx = PREV_NET_TX.get().expect("PREV_NET_TX").lock().unwrap();

    let delta_rx = current_rx.saturating_sub(*prev_rx);
    let delta_tx = current_tx.saturating_sub(*prev_tx);

    // 5. 更新累计值
    *prev_rx = current_rx;
    *prev_tx = current_tx;

    // 6. 计算速率
    let download_speed = delta_rx as f64 / elapsed;
    let upload_speed = delta_tx as f64 / elapsed;

    (download_speed, upload_speed)
}
