# Star Shuttle

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)

Star Shuttle 是一个基于 Tauri 2、Svelte 5 和 Rust 的跨平台 SSH 远程管理桌面应用。它面向需要频繁连接服务器、管理远程终端和传输文件的开发者、运维人员与技术团队。

项目目标是在轻量桌面客户端中提供连接管理、交互式终端、SFTP/SCP 文件操作、会话状态管理以及可扩展的 AI 辅助能力。

## 主要功能

- SSH 连接管理：创建、编辑、删除和组织远程主机配置。
- 多会话终端：基于 xterm.js 的远程终端体验，支持标签、分屏和会话恢复相关能力。
- 文件传输：通过 SFTP/SCP 浏览、上传、下载和管理远程文件。
- 凭证与主机信任：包含认证、已知主机、凭证同步和连接探测等后端模块。
- 本地配置与持久化：使用 SQLite 和本地配置模块保存应用状态。
- 命令片段与快捷操作：提供常用命令管理和前端快捷入口。
- AI 辅助模块：包含聊天、Agent、技能管理、上下文收集和命令执行工具等实验性能力。

## 技术栈

- 桌面框架：Tauri 2
- 前端：Svelte 5、TypeScript、Vite、Tailwind CSS
- 终端：xterm.js 及 Fit、Search、Web Links、WebGL 插件
- 后端：Rust 
- 数据存储：SQLite


## 快速开始

### 环境要求

- Node.js 18+
- Rust stable
- Tauri 2 所需的系统依赖

Linux 环境通常还需要安装 WebKitGTK、GTK、OpenSSL、构建工具等依赖。具体可参考 Tauri 官方文档或 [doc/deployment_guide.md](doc/deployment_guide.md)。

### 安装依赖

```bash
npm install
```

### 启动前端开发服务

```bash
npm run dev
```

### 启动完整桌面应用

```bash
npx tauri dev
```

### 构建

```bash
npm run build
npx tauri build
```

```
建议使用wsl进行跨平台编译

https://v2.tauri.org.cn/distribute/windows-installer/
```

Rust 后端测试需要在 `src-tauri/` 目录执行：

```bash
cd src-tauri
cargo test
```

## 项目结构

```text
src/                         Svelte 前端入口、状态、服务和 UI
src/components/              终端、布局、连接、文件传输、AI 等组件
src/lib/                     前端服务封装、工具函数和 Vitest 测试
src-tauri/src/               Rust 后端入口和 Tauri 命令
src-tauri/src/modules/       SSH、SFTP、终端、数据库、AI 等后端模块
doc/                         架构、需求、部署、测试和评审文档
```

## SKILLS

- [x] 运维skill
- [x] 应急响应skills

## 文档

- [功能规范](doc/functional_specification.md)
- [软件架构](doc/software_architecture.md)
- [API 文档](doc/api_documentation.md)
- [部署指南](doc/deployment_guide.md)
- [测试报告](doc/test_report.md)

## 安全说明

Star Shuttle 涉及 SSH 凭证、主机密钥、本地文件系统访问和远程命令执行。修改以下区域时需要额外谨慎：

- `src-tauri/capabilities/default.json`
- `src-tauri/src/modules/connection/`
- `src-tauri/src/modules/credential/`
- `src-tauri/src/modules/sftp/`
- `src-tauri/src/modules/ai/tools/execute_command.rs`

建议在提交涉及连接生命周期、凭证存储、known_hosts、SFTP 缓冲或 Tauri 权限的变更前，补充对应测试并进行安全复查。

## 贡献

欢迎围绕连接稳定性、终端体验、文件传输可靠性、测试覆盖和跨平台兼容性继续改进。提交前建议运行：

```bash
npm run format
npm run lint
npm run check
npm test
cd src-tauri && cargo test
```

## 许可证

本项目使用 GPL-3.0-or-later 许可证发布，详见 [LICENSE](LICENSE)。
