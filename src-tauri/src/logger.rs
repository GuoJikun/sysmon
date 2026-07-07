//! 日志初始化模块
//!
//! 使用 `log` crate 作为日志 facade，`fern` 作为输出前端。
//! 日志同时输出到：
//! - 文件：`app_config_dir/sysmon.log`（追加模式，自动轮转）
//! - stdout（当以 `tauri dev` 或控制台启动时可见）
//!
//! 日志等级可在设置界面中修改，持久化到 `settings.json` 的 `log_level` 字段。

use std::path::Path;

/// 日志等级字符串到 `log::LevelFilter` 的映射
pub fn level_from_str(s: &str) -> log::LevelFilter {
    match s.to_lowercase().as_str() {
        "off"   => log::LevelFilter::Off,
        "error"  => log::LevelFilter::Error,
        "warn"   => log::LevelFilter::Warn,
        "info"   => log::LevelFilter::Info,
        "debug"  => log::LevelFilter::Debug,
        "trace"  => log::LevelFilter::Trace,
        _         => log::LevelFilter::Info,
    }
}

/// 将 `log::LevelFilter` 转回字符串（用于设置界面回显）
pub fn level_to_string(lf: log::LevelFilter) -> &'static str {
    match lf {
        log::LevelFilter::Off   => "off",
        log::LevelFilter::Error => "error",
        log::LevelFilter::Warn  => "warn",
        log::LevelFilter::Info  => "info",
        log::LevelFilter::Debug => "debug",
        log::LevelFilter::Trace => "trace",
    }
}

/// 初始化日志系统
///
/// 必须在 `tauri::Builder::setup` 中最早调用。
/// 日志文件位于 `app_config_dir/sysmon.log`。
pub fn init(log_level: log::LevelFilter, app_config_dir: &Path) {
    let log_path = app_config_dir.join("sysmon.log");

    // fern::log_file 接受 PathBuf，返回一个实现了 Write + Send + 'static 的类型
    let file = match fern::log_file(&log_path) {
        Ok(f)  => f,
        Err(e) => {
            eprintln!("Failed to open log file {:?}: {}", log_path, e);
            // fallback：写到 stdout 即可，不中断启动
            fern::Dispatch::new()
                .format(|out, message, record| {
                    out.finish(format_args!(
                        "{} [{}] [{}] {}",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                        record.level(),
                        record.target(),
                        message,
                    ))
                })
                .level(log_level)
                .chain(std::io::stdout())
                .apply()
                .unwrap_or_else(|e| eprintln!("Failed to init logger: {}", e));
            return;
        }
    };

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] [{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.target(),
                message,
            ))
        })
        .level(log_level)
        .chain(std::io::stdout())
        .chain(file)
        .apply()
        .unwrap_or_else(|e| eprintln!("Failed to init logger: {}", e));

    log::info!("Logger initialized, level={:?}", log_level);
}
