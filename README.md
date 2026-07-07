# SysMon — Windows 系统监控工具

[English](README.en.md) | **中文**

轻量级 Windows 桌面系统监控工具，悬浮置顶显示 CPU、内存、网速，支持任务栏内嵌网速条。

## 功能特性

- **悬浮监控面板** — 无边框置顶窗口，紧凑显示 CPU/内存/网速，不占用任务栏
- **任务栏网速条** — 通过 Win32 API 嵌入 Windows 任务栏，实时显示上传/下载速度
- **GPU 监控** — 基于 Windows PDH API 采集 GPU 3D 引擎利用率
- **系统托盘** — 后台运行，右键菜单可显示主窗口、打开设置、退出
- **主题切换** — 浅色/深色双主题，设置窗口统一管理
- **设置持久化** — 任务栏显示、窗口置顶等配置自动保存

## 技术栈

| 组件 | 技术 |
|------|------|
| 桌面框架 | Tauri v2 |
| 后端 | Rust + sysinfo + Windows API (PDH, Win32) |
| 前端 | 原生 HTML / CSS / JS（无框架） |
| 异步 | Tokio |

## 项目结构

```
sysmon/
├── src/                    # 前端
│   ├── main.html/js        # 主监控窗口
│   ├── taskbar.html/js     # 任务栏网速窗口
│   ├── settings.html/js    # 设置窗口
│   └── styles/             # 样式（含主题变量）
└── src-tauri/
    ├── tauri.conf.json     # 应用配置
    ├── Cargo.toml          # Rust 依赖
    ├── capabilities/       # 窗口权限配置
    └── src/
        ├── lib.rs          # 应用入口、事件系统、定时器
        ├── sys_info.rs     # CPU/内存/网络采集
        ├── gpu.rs          # GPU 监控（PDH API）
        ├── tray.rs         # 系统托盘
        ├── taskbar_window.rs # 任务栏窗口嵌入
        ├── commands.rs     # 数据结构
        └── settings.rs     # 设置持久化
```

## 快速开始

### 环境要求

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+
- [pnpm](https://pnpm.io/)
- Windows 10/11

### 安装依赖

```bash
pnpm install
```

### 开发运行

```bash
cargo tauri dev
```

### 构建发布

```bash
cargo tauri build
```

## 使用说明

### 主窗口

启动后主窗口悬浮在屏幕中央，显示：
```
上传: 0.28 KB/s   下载: 0.61 KB/s
CPU: 34%           内存: 70%
```

- 窗口始终置顶，可拖拽移动
- 关闭窗口 = 隐藏到托盘（不退出）

### 系统托盘

右键托盘图标：
- **显示主窗口** — 显示并聚焦主窗口
- **设置** — 打开设置窗口
- **退出** — 退出程序

左键点击托盘图标 = 显示主窗口。

### 设置

| 设置项 | 说明 | 默认值 |
|--------|------|--------|
| 主题 | 浅色 / 深色 | 浅色 |
| 任务栏显示 | 显示 / 隐藏任务栏网速条 | 隐藏 |
| 窗口置顶 | 开启 / 关闭主窗口置顶 | 开启 |

设置保存在 `%APPDATA%\com.sysmon.app\settings.json`。

## 架构概览

```
Windows API (sysinfo + PDH)
        │
   Rust 后端 (1.5s 定时采集)
        │
   ┌────┴────┐
   ▼         ▼
 主窗口    任务栏窗口
(CPU/内存  (网速条)
 /网速)
```

- 后端每 1.5 秒采集系统信息，通过 Tauri 事件推送到前端
- 任务栏窗口通过 `SetParent` Win32 API 嵌入 Windows 任务栏
- 主题变更通过 `emit('theme-changed')` 广播到所有窗口
- 设置通过 `invoke()` 调用 Rust 命令持久化

## License

MIT
