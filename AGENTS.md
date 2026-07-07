# AGENTS.md — SysMon 项目 AI Agent 指南

> 本文件供 AI Agent（如 WorkBuddy、Cursor、Copilot 等）阅读，帮助快速理解项目架构、编码规范和关键约束。

## 项目概述

SysMon 是一个 **Windows 桌面系统监控工具**，基于 Tauri v2 + Rust + 原生 HTML/CSS/JS 构建。

核心功能：
- 实时监控 CPU、内存、GPU 使用率
- 实时网速监控（上传/下载）
- 主窗口悬浮置顶显示（无边框、紧凑布局）
- 任务栏内嵌网速小窗口（通过 Win32 API SetParent 嵌入）
- 系统托盘后台运行
- 浅色/深色主题切换
- 设置持久化（任务栏显示、窗口置顶）

## 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| 桌面框架 | Tauri v2 | Rust 后端 + WebView 前端 |
| 后端语言 | Rust | 系统信息采集、Win32 API 调用 |
| 前端 | 原生 HTML/CSS/JS | 无框架，无构建步骤 |
| 系统信息 | sysinfo crate | CPU/内存/网络 |
| GPU 监控 | Windows PDH API | `\GPU Engine(*)\Utilization Percentage` |
| 任务栏嵌入 | Win32 API | SetParent / ReBarWindow32 |
| 异步运行时 | Tokio | 定时器、异步任务 |

## 项目结构

```
sysmon/
├── src/                          # 前端源码
│   ├── main.html / main.js       # 主监控窗口
│   ├── taskbar.html / taskbar.js # 任务栏网速窗口
│   ├── settings.html / settings.js # 设置窗口
│   └── styles/
│       ├── variables.css         # CSS 变量（主题定义）
│       ├── public.css            # 公共样式
│       ├── main.css              # 主窗口样式
│       ├── taskbar.css           # 任务栏窗口样式
│       └── settings.css          # 设置窗口样式
├── src-tauri/
│   ├── tauri.conf.json           # Tauri 应用配置
│   ├── Cargo.toml                # Rust 依赖
│   ├── capabilities/             # Tauri v2 权限配置
│   │   ├── default.json          # 主窗口权限
│   │   ├── taskbar.json          # 任务栏窗口权限
│   │   └── settings.json         # 设置窗口权限
│   └── src/
│       ├── main.rs               # 入口（6 行，调用 lib::run()）
│       ├── lib.rs                # 应用配置、事件系统、定时器
│       ├── sys_info.rs           # CPU/内存/网络采集
│       ├── gpu.rs                # GPU 监控（PDH API）
│       ├── tray.rs               # 系统托盘
│       ├── taskbar_window.rs     # 任务栏窗口嵌入（Win32 API）
│       ├── commands.rs           # 数据结构定义
│       └── settings.rs           # 设置持久化
└── package.json                  # 前端依赖（仅 @tauri-apps/cli）
```

## 架构与数据流

### 数据采集与推送

```
Windows API (sysinfo crate + PDH)
        │
        ▼
  Rust 后端 (每 1.5s 采集)
        │
        ├── emit_to("main", "sys-info", SystemInfo)
        │         └── CPU%, 内存%, 网速
        │
        └── emit_to("taskbar", "net-speed", NetSpeedInfo)
                  └── 上传/下载速度
```

### 窗口体系

| 窗口 label | 用途 | 创建方式 | 关闭行为 |
|------------|------|----------|----------|
| `main` | 悬浮监控面板 | tauri.conf.json 静态配置 | 隐藏不退出 |
| `taskbar` | 任务栏网速条 | taskbar_window.rs 动态创建 | 阻止关闭 |
| `settings` | 设置面板 | tray.rs 动态创建 | 允许正常关闭 |

### 窗口间通信

- **主题同步**：settings 窗口 `emit('theme-changed', theme)` → main/taskbar 窗口 `listen` 同步
- **设置持久化**：settings 窗口 `invoke()` 调用 Rust 命令 → 写入 `%APPDATA%\com.sysmon.app\settings.json`
- **数据推送**：Rust 定时器 `emit_to()` → 各窗口 `listen` 更新 DOM

## 编码规范

### Rust 后端
- 使用 `OnceLock<Mutex<T>>` 管理全局状态（系统信息采集器、网络累计值）
- Win32 API 调用使用 `windows` crate，unsafe 块需注释说明
- Tauri 命令使用 `#[tauri::command]` 宏，返回值需实现 `Serialize`
- 错误处理：用 `Result` 和 `?`，非关键错误用 `eprintln!` 记录后继续

### 前端
- 使用 Tauri 全局 API（`window.__TAURI__`），无需 import
- DOM 操作直接用 `document.getElementById`
- 主题通过 `data-theme` 属性 + CSS 变量实现
- 不使用任何前端框架或构建工具

### 样式
- CSS 变量定义在 `variables.css`，浅色/深色双主题
- `data-theme="light"` / `data-theme="dark"` 切换
- 窗口无边框时用 `.drag-overlay`（`data-tauri-drag-region`）实现拖拽

## 关键约束

1. **Tauri v2 权限**：每个窗口的 IPC 调用需在 `capabilities/*.json` 中显式授权
   - 拖拽需要 `core:window:allow-start-dragging`
   - 置顶需要 `core:window:allow-set-always-on-top`
   - 隐藏/显示需要 `core:window:allow-hide` / `allow-show`

2. **GPU 监控**：PDH 需要两次 `CollectQueryData` 才有真实值，`init_gpu_monitor` 中预采集一次

3. **网速计算**：增量算法，需过滤虚拟网卡（Loopback、vEthernet、Hyper-V、docker）

4. **任务栏嵌入**：退出时必须恢复 `MSTaskSwWClass` 原始尺寸，否则任务栏会永久变小

5. **窗口高度**：主窗口当前为紧凑布局（230×60），修改布局时需同步调整 `tauri.conf.json` 中的尺寸

## 开发命令

```bash
# 开发模式（热重载）
cargo tauri dev

# 编译检查（不生成二进制）
cd src-tauri && cargo check

# 构建发布版本
cargo tauri build
```

## 常见任务指引

### 添加新的监控指标
1. `commands.rs` 中扩展 `SystemInfo` 结构体
2. `sys_info.rs` 中添加采集逻辑
3. `lib.rs` 的 `start_data_push_timer` 中传入新数据
4. `main.html` / `main.js` 中添加 DOM 和更新逻辑

### 添加新的设置项
1. `settings.rs` 的 `AppSettings` 结构体添加字段
2. 添加 `get_xxx` / `set_xxx` Tauri 命令
3. 添加 `apply_xxx_setting` 启动应用函数
4. `lib.rs` 注册命令并调用 apply 函数
5. `settings.html` / `settings.js` 添加 UI 和交互
6. 如涉及窗口操作，在 `capabilities/default.json` 添加权限

### 修改主窗口布局
1. `src/main.html` 修改 DOM 结构
2. `src/styles/main.css` 修改样式
3. `src-tauri/tauri.conf.json` 同步调整窗口尺寸
