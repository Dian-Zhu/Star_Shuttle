# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

Star Shuttle 是一个基于 Tauri 2、Svelte 5 和 Rust 的跨平台 SSH 远程管理桌面应用。它提供连接管理、交互式终端（xterm.js）、SFTP/SCP 文件传输、凭证与 known_hosts 处理，以及实验性的 AI 助手（chat 与 agent 两种模式）。数据持久化使用本地 SQLite。界面文案与文档以中文为主，编辑时请与周围语言保持一致。

## 常用命令

前端（在仓库根目录执行）：

```bash
npm run dev        # Vite 开发服务器（仅前端，端口 5173）
npx tauri dev      # 完整桌面应用（通过 beforeDevCommand 先运行 npm run dev）
npm run build      # vite build -> dist/
npx tauri build    # 打包桌面应用（建议在 WSL 中交叉编译 Windows 版本）
npm test           # vitest run（运行全部前端测试）
npx vitest run src/lib/terminalService.test.ts   # 运行单个测试文件
npm run check      # svelte-check 类型检查
npm run lint       # eslint（--max-warnings 0，警告即失败）
npm run format     # prettier --write .
```

Rust 后端（在 `src-tauri/` 目录执行）：

```bash
cd src-tauri
cargo test                          # 运行全部后端测试
cargo test --package <pkg> <name>   # 按名称子串运行单个测试
cargo build
```

Vitest 在 `node` 环境下运行，测试间会自动 clear/reset/restore mock；`@tauri-apps/api` 的调用（`invoke`、`listen`）在 `src/lib/test/vitest.setup.ts` 中被 mock。

## 架构

### 进程划分

本应用是一个 Tauri 应用：Svelte/TypeScript 前端（`src/`）通过 Tauri 的 IPC 与 Rust 后端（`src-tauri/src/`）通信。后端暴露约 83 个 `#[tauri::command]` 函数，全部注册在 `src-tauri/src/lib.rs` 的 `invoke_handler!` 宏中。后端托管的状态（连接管理器、SFTP 管理器、数据库、AI 管理器、应用锁、截图）在 `lib.rs` 的 Tauri `.setup()` 块中通过 `app.manage(...)` 初始化，并以 `State<'_, Arc<Mutex<...>>>` 的形式注入到命令中。新增命令时，需要在 handler 中注册，并（通常）在 `src/lib/` 中封装对应的前端服务调用。

### 多窗口路由

`src/App.svelte` 根据 Tauri 窗口 label 进行路由：`main` 加载完整应用（`MainApp.svelte` → `Layout.svelte`），`screenshot-overlay` 加载截图区域选择器，`pin-*` 加载钉住的截图窗口。overlay/pin 窗口懒加载各自的组件，从而不会引入 xterm/终端相关的打包体积。

### 终端生命周期（关键设计）

终端实例的生命周期被刻意与 Svelte 组件生命周期解耦，使终端能在切换标签、重新布局和重连时保持存活：

- `TerminalPool`（`src/lib/terminalPool.ts`）是以 `sessionId` 为键的全局单例，持有所有 `TerminalInstance`。组件不直接拥有实例。
- `TerminalInstance`（`terminalInstance.ts`）封装 xterm.js 的 `Terminal` 及插件（fit、search、webgl、web-links）。
- `TerminalProxy`（`terminalProxy.ts`）是组件作用域内对实例的门面（facade），其生命周期绑定组件，但不影响底层实例。
- `terminalService.ts` 编排连接/重连/广播，把 Tauri 事件（SSH 输出）桥接到实例，并驱动 Svelte store。

前端状态位于 `src/lib/store.ts` 的 Svelte store 中（连接、活动终端、会话映射、设置、广播状态）。`terminalSessionModel.ts` 定义会话状态的 patch/merge 模型。

### 后端模块（`src-tauri/src/modules/`）

- `connection/` — 最大的子系统：SSH（russh）连接流程、认证方式（密码、密钥、keyboard-interactive、agent、证书）、代理（SOCKS5/HTTP/跳板机）、端口转发、known_hosts、Telnet 以及终端执行。流程拆分在 `flow.rs`（prepare/finish 阶段）、`ssh_connect.rs`、`ssh_impl*.rs` 和 `terminal_control.rs`/`terminal_exec.rs` 中。
- `sftp/` — SFTP + SCP 文件操作、会话缓存及 Tauri 命令。
- `terminal/` — 服务端终端 emulator/parser/buffer。
- `db/` — 基于 rusqlite（bundled）的 SQLite。`DatabaseManager` 持有连接；`ai_store.rs` 与 `command_snippets.rs` 通过各自的 `create_tables` 创建自己的表。已启用外键（`PRAGMA foreign_keys = ON`）以支持级联删除。
- `ai/` — AI 助手。`chat.rs`（ChatManager）用于 chat 模式；`agent.rs`/`orchestrator.rs`/`planner.rs` 用于 agent 模式（plan→act 循环，`MAX_PLANNER_STEPS = 20`）；`tools/` 是可被 agent 调用的工具（`execute_command.rs`、`get_system_info.rs`），受 `ToolRegistry` + `ToolAuthorization` 和 `SandboxMode` 约束；`skills.rs` 管理可安装的 skill，用于限定允许的工具集。
- `local_fs/` — 通过 access-token grant/handle 实现沙箱化的本地文件系统访问。
- `app_runtime/` — 应用锁（bcrypt）与 host-key 挑战的运行时状态。
- `screenshot/`、`session/`、`config/`、`credential/`、`logging/`、`error/` — 支撑性模块。

## 安全敏感区域

修改以下区域时需格外谨慎（并补充测试）：`src-tauri/capabilities/*.json`（Tauri 权限）、`connection/`（连接生命周期、凭证、known_hosts）、`credential/`、`sftp/`，以及 `ai/tools/execute_command.rs`（远程命令执行）。该应用涉及 SSH 凭证、主机密钥、本地文件系统访问和远程命令执行。`src-tauri/tauri.conf.json` 中的 CSP 很严格（`script-src 'self'`），请保持其收紧状态。

## 文档

`doc/` 存放架构、需求、部署、AI-agent 设计及评审报告（如 `software_architecture.md`、`ai_agent_architecture.md`、`api_documentation.md`、`deployment_guide.md`）。
