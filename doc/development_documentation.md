# SSH 远程管理工具 - 开发文档

## 1. 文档信息

| 项目     | 内容                         |
| -------- | ---------------------------- |
| 文档名称 | SSH 远程管理工具 - 开发文档  |
| 文档版本 | 1.0                          |
| 文档作者 | 开发团队                     |
| 创建日期 | 2026-01-14                   |
| 更新日期 | 2026-01-14                   |
| 适用范围 | 开发团队、测试团队、维护团队 |

## 2. 项目概述

### 2.1 项目简介

SSH 远程管理工具是一款基于 Rust 和 Tauri 2 框架开发的跨平台桌面应用，提供 SSH 连接管理、远程终端访问、文件传输等功能。该工具旨在为用户提供一个安全、高效、易用的远程服务器管理解决方案，支持 Windows、macOS 和 Linux 平台。

### 2.2 核心功能

1. **SSH 连接管理**：创建、编辑、删除、组织 SSH 连接配置
2. **多种认证方式**：支持密码认证、私钥认证、SSH 代理转发
3. **远程终端仿真**：提供功能完整的终端界面，支持 ANSI 颜色、鼠标操作等
4. **文件传输**：支持 SFTP/SCP 协议，提供双面板文件浏览器和拖拽操作
5. **会话管理**：支持多会话并发管理、会话保存和恢复
6. **连接历史和日志**：记录连接历史和操作日志

### 2.3 技术栈

| 技术/框架  | 版本  | 用途               |
| ---------- | ----- | ------------------ |
| Rust       | 1.70+ | 后端开发语言       |
| Tauri      | 2.0   | 跨平台桌面应用框架 |
| Svelte     | 4.0+  | 前端 UI 框架       |
| russh      | 0.40+ | SSH 协议实现       |
| russh-sftp | 0.40+ | SFTP 协议实现      |
| keyring    | 2.0+  | 平台原生安全存储   |
| rusqlite   | 0.29+ | SQLite 数据库访问  |
| xterm.js   | 5.0+  | 前端终端仿真库     |

## 3. 系统架构

### 3.1 整体架构

系统采用分层架构模式，结合 Tauri 2 的主进程-渲染进程模型，主要分为以下三层：

1. **前端层**（渲染进程）：使用 Web 技术实现用户界面，包括连接管理、终端模拟器、文件传输界面等
2. **中间层**（IPC 通信）：负责前端和后端之间的通信，基于 Tauri 提供的 IPC 机制
3. **后端层**（主进程）：使用 Rust 实现核心业务逻辑，包括 SSH 连接管理、终端仿真、文件传输等

### 3.2 Tauri 应用架构

Tauri 2 应用采用双进程模型：

- **主进程**：使用 Rust 编写，运行在系统原生环境中，具有完全的系统访问权限，负责处理敏感操作和核心业务逻辑
- **渲染进程**：使用 Web 技术（HTML/CSS/JavaScript）编写，运行在安全的沙箱环境中，负责用户界面渲染和用户交互

### 3.3 组件交互图

```
┌───────────────────────────────────────────────────────────────────────────┐
│                                 前端层 (渲染进程)                          │
├─────────────────┬─────────────────┬─────────────────┬─────────────────────┤
│ 连接管理界面    │ 终端模拟器 UI    │ 文件传输界面     │ 设置面板             │
├─────────────────┴─────────────────┴─────────────────┴─────────────────────┤
│                                 IPC 通信层                                │
├─────────────────┬─────────────────┬─────────────────┬─────────────────────┤
│ SSH 连接管理器  │ 终端仿真引擎     │ SFTP/SCP 引擎    │ 凭证安全存储         │
├─────────────────┼─────────────────┼─────────────────┼─────────────────────┤
│ 会话管理模块    │ 日志记录模块     │ 配置管理模块     │ 错误处理模块         │
└─────────────────┴─────────────────┴─────────────────┴─────────────────────┘
```

## 4. 模块划分

### 4.1 前端模块

#### 4.1.1 连接管理模块

**功能**：管理 SSH 连接配置

**职责**：

- 连接列表展示和搜索
- 连接创建、编辑、删除
- 连接分组和组织
- 连接状态监控

**文件结构**：

```
frontend/src/components/connection/
├── ConnectionList.svelte      # 连接列表组件
├── ConnectionForm.svelte      # 连接创建/编辑表单
├── ConnectionGroup.svelte     # 连接分组组件
└── ConnectionStatus.svelte    # 连接状态组件
```

#### 4.1.2 终端模拟器模块

**功能**：提供远程终端界面

**职责**：

- 终端显示和输入处理
- 终端大小调整
- 终端主题和字体设置
- 复制粘贴功能

**文件结构**：

```
frontend/src/components/terminal/
├── Terminal.svelte            # 终端主组件
├── TerminalToolbar.svelte     # 终端工具栏
├── TerminalTabs.svelte        # 终端标签页
└── TerminalSettings.svelte    # 终端设置
```

#### 4.1.3 文件传输模块

**功能**：提供本地与远程文件系统交互

**职责**：

- 双面板文件浏览器
- 文件上传下载
- 目录操作（创建、删除、重命名）
- 传输队列管理

**文件结构**：

```
frontend/src/components/file-transfer/
├── FileBrowser.svelte         # 双面板文件浏览器
├── FileTransferQueue.svelte   # 传输队列
├── FileOperations.svelte      # 文件操作工具栏
└── TransferProgress.svelte    # 传输进度
```

#### 4.1.4 设置模块

**功能**：管理应用配置和用户偏好

**职责**：

- 通用设置
- 终端设置
- 文件传输设置
- 安全设置

**文件结构**：

```
frontend/src/components/settings/
├── SettingsPanel.svelte       # 设置面板主组件
├── GeneralSettings.svelte     # 通用设置
├── TerminalSettings.svelte    # 终端设置
├── FileTransferSettings.svelte # 文件传输设置
└── SecuritySettings.svelte    # 安全设置
```

### 4.2 后端模块

#### 4.2.1 SSH 连接管理器

**功能**：管理 SSH 连接的生命周期

**职责**：

- 连接配置验证和解析
- 多种认证方式支持
- 连接池管理
- 连接状态监控

**文件结构**：

```
src/modules/connection/
├── mod.rs                     # 模块入口
├── manager.rs                 # 连接管理器实现
├── config.rs                  # 连接配置定义
├── auth.rs                    # 认证处理
└── error.rs                   # 错误定义
```

#### 4.2.2 终端仿真引擎

**功能**：处理终端输入输出，模拟终端行为

**职责**：

- 终端输入处理和输出渲染
- ANSI/VT100 转义序列解析
- 终端窗口大小调整处理
- 滚动缓冲区管理

**文件结构**：

```
src/modules/terminal/
├── mod.rs                     # 模块入口
├── emulator.rs                # 终端仿真实现
├── parser.rs                  # ANSI 转义序列解析
├── buffer.rs                  # 滚动缓冲区管理
└── error.rs                   # 错误定义
```

#### 4.2.3 SFTP/SCP 文件传输引擎

**功能**：处理本地与远程系统之间的文件传输

**职责**：

- SFTP/SCP 协议实现
- 文件上传和下载
- 目录操作
- 文件权限管理
- 传输队列管理

**文件结构**：

```
src/modules/file_transfer/
├── mod.rs                     # 模块入口
├── engine.rs                  # 文件传输引擎
├── sftp.rs                    # SFTP 协议实现
├── scp.rs                     # SCP 协议实现
├── queue.rs                   # 传输队列管理
└── error.rs                   # 错误定义
```

#### 4.2.4 凭证安全存储模块

**功能**：安全存储和管理 SSH 凭证信息

**职责**：

- 密码加密存储
- 私钥安全管理
- 平台原生安全存储集成
- 凭证访问控制

**文件结构**：

```
src/modules/credential/
├── mod.rs                     # 模块入口
├── store.rs                   # 凭证存储实现
└── error.rs                   # 错误定义
```

#### 4.2.5 会话管理模块

**功能**：管理用户会话状态

**职责**：

- 会话状态保存和恢复
- 多会话并发管理
- 会话配置管理

**文件结构**：

```
src/modules/session/
├── mod.rs                     # 模块入口
├── manager.rs                 # 会话管理器实现
└── error.rs                   # 错误定义
```

#### 4.2.6 日志记录模块

**功能**：记录系统运行日志

**职责**：

- 连接历史记录
- 终端会话日志
- 文件传输日志
- 错误和事件日志

**文件结构**：

```
src/modules/logging/
├── mod.rs                     # 模块入口
├── logger.rs                  # 日志记录器实现
├── formatter.rs               # 日志格式化
└── storage.rs                 # 日志存储
```

## 5. 接口定义

### 5.1 前端-后端 IPC API

#### 5.1.1 连接管理 API

| 方法名             | 参数                                                                                   | 返回值                                                    | 描述             |
| ------------------ | -------------------------------------------------------------------------------------- | --------------------------------------------------------- | ---------------- |
| `connect`          | `{ id: string, host: string, port: number, username: string, authMethod: AuthMethod }` | `{ success: boolean, sessionId: string, error?: string }` | 建立 SSH 连接    |
| `disconnect`       | `{ sessionId: string }`                                                                | `{ success: boolean, error?: string }`                    | 断开 SSH 连接    |
| `getConnections`   | -                                                                                      | `{ connections: Connection[] }`                           | 获取所有连接配置 |
| `saveConnection`   | `{ connection: Connection }`                                                           | `{ success: boolean, id: string, error?: string }`        | 保存连接配置     |
| `deleteConnection` | `{ id: string }`                                                                       | `{ success: boolean, error?: string }`                    | 删除连接配置     |
| `testConnection`   | `{ host: string, port: number, username: string, authMethod: AuthMethod }`             | `{ success: boolean, error?: string }`                    | 测试 SSH 连接    |

#### 5.1.2 终端管理 API

| 方法名           | 参数                                                 | 返回值                                                     | 描述           |
| ---------------- | ---------------------------------------------------- | ---------------------------------------------------------- | -------------- |
| `createTerminal` | `{ sessionId: string }`                              | `{ success: boolean, terminalId: string, error?: string }` | 创建终端会话   |
| `writeTerminal`  | `{ terminalId: string, data: string }`               | `{ success: boolean, error?: string }`                     | 向终端写入数据 |
| `resizeTerminal` | `{ terminalId: string, cols: number, rows: number }` | `{ success: boolean, error?: string }`                     | 调整终端大小   |
| `closeTerminal`  | `{ terminalId: string }`                             | `{ success: boolean, error?: string }`                     | 关闭终端会话   |

#### 5.1.3 文件传输 API

| 方法名              | 参数                                                           | 返回值                                                         | 描述             |
| ------------------- | -------------------------------------------------------------- | -------------------------------------------------------------- | ---------------- |
| `listDirectory`     | `{ sessionId: string, path: string }`                          | `{ success: boolean, entries: FileEntry[], error?: string }`   | 列出目录内容     |
| `uploadFile`        | `{ sessionId: string, localPath: string, remotePath: string }` | `{ success: boolean, transferId: string, error?: string }`     | 上传文件         |
| `downloadFile`      | `{ sessionId: string, remotePath: string, localPath: string }` | `{ success: boolean, transferId: string, error?: string }`     | 下载文件         |
| `createDirectory`   | `{ sessionId: string, path: string }`                          | `{ success: boolean, error?: string }`                         | 创建目录         |
| `deleteFile`        | `{ sessionId: string, path: string }`                          | `{ success: boolean, error?: string }`                         | 删除文件或目录   |
| `renameFile`        | `{ sessionId: string, oldPath: string, newPath: string }`      | `{ success: boolean, error?: string }`                         | 重命名文件或目录 |
| `getTransferStatus` | `{ transferId: string }`                                       | `{ success: boolean, status: TransferStatus, error?: string }` | 获取传输状态     |
| `cancelTransfer`    | `{ transferId: string }`                                       | `{ success: boolean, error?: string }`                         | 取消传输         |

#### 5.1.4 事件定义

| 事件名                      | 数据                                                       | 描述         |
| --------------------------- | ---------------------------------------------------------- | ------------ |
| `terminal-data`             | `{ terminalId: string, data: string }`                     | 终端输出数据 |
| `terminal-closed`           | `{ terminalId: string }`                                   | 终端会话关闭 |
| `connection-status-changed` | `{ sessionId: string, status: ConnectionStatus }`          | 连接状态变更 |
| `transfer-progress`         | `{ transferId: string, progress: number, speed: number }`  | 传输进度更新 |
| `transfer-completed`        | `{ transferId: string, success: boolean, error?: string }` | 传输完成     |

### 5.2 模块内部接口

#### 5.2.1 SSH 连接管理器接口

```rust
trait ConnectionManager {
    fn connect(&mut self, config: ConnectionConfig) -> Result<SessionId, ConnectionError>;
    fn disconnect(&mut self, session_id: &SessionId) -> Result<(), ConnectionError>;
    fn get_session(&self, session_id: &SessionId) -> Option<&Session>;
    fn get_all_sessions(&self) -> Vec<SessionInfo>;
}
```

#### 5.2.2 终端仿真引擎接口

```rust
trait TerminalEmulator {
    fn new(session_id: SessionId) -> Self;
    fn write(&mut self, data: &[u8]) -> Result<(), TerminalError>;
    fn read(&mut self) -> Vec<u8>;
    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), TerminalError>;
    fn close(&mut self) -> Result<(), TerminalError>;
}
```

#### 5.2.3 文件传输引擎接口

```rust
trait FileTransferEngine {
    fn list_directory(&self, session_id: &SessionId, path: &str) -> Result<Vec<FileEntry>, TransferError>;
    fn upload(&self, session_id: &SessionId, local_path: &str, remote_path: &str) -> Result<TransferId, TransferError>;
    fn download(&self, session_id: &SessionId, remote_path: &str, local_path: &str) -> Result<TransferId, TransferError>;
    fn get_transfer_status(&self, transfer_id: &TransferId) -> Result<TransferStatus, TransferError>;
}
```

## 6. 数据模型

### 6.1 连接配置模型

```typescript
interface Connection {
  id: string
  name: string
  host: string
  port: number
  username: string
  authMethod: AuthMethod
  description?: string
  tags?: string[]
  createdAt: Date
  updatedAt: Date
}

type AuthMethod =
  | { type: 'password'; password: string; savePassword: boolean }
  | { type: 'privateKey'; keyPath: string; passphrase?: string; savePassphrase: boolean }
  | { type: 'agent'; agentPath?: string }
```

### 6.2 会话模型

```typescript
interface Session {
  id: string
  connectionId: string
  status: ConnectionStatus
  terminalId?: string
  createdAt: Date
  lastActive: Date
}

type ConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'disconnecting' | 'error'
```

### 6.3 文件条目模型

```typescript
interface FileEntry {
  name: string
  path: string
  isDirectory: boolean
  size: number
  modified: Date
  permissions: string
  owner: string
  group: string
}
```

### 6.4 传输状态模型

```typescript
interface TransferStatus {
  id: string
  type: 'upload' | 'download'
  sessionId: string
  localPath: string
  remotePath: string
  progress: number // 0-100
  speed: number // bytes per second
  status: 'pending' | 'transferring' | 'completed' | 'failed' | 'canceled'
  error?: string
  startTime: Date
  endTime?: Date
}
```

### 6.5 数据库结构

#### 6.5.1 连接表 (connections)

| 字段名      | 数据类型 | 约束        | 描述                  |
| ----------- | -------- | ----------- | --------------------- |
| id          | TEXT     | PRIMARY KEY | 连接唯一标识符        |
| name        | TEXT     | NOT NULL    | 连接名称              |
| host        | TEXT     | NOT NULL    | 主机名或 IP 地址      |
| port        | INTEGER  | NOT NULL    | SSH 端口              |
| username    | TEXT     | NOT NULL    | 用户名                |
| auth_method | TEXT     | NOT NULL    | 认证方式              |
| auth_config | TEXT     | NOT NULL    | 认证配置（JSON 格式） |
| description | TEXT     |             | 连接描述              |
| tags        | TEXT     |             | 标签（JSON 格式）     |
| created_at  | INTEGER  | NOT NULL    | 创建时间戳            |
| updated_at  | INTEGER  | NOT NULL    | 更新时间戳            |

#### 6.5.2 会话表 (sessions)

| 字段名        | 数据类型 | 约束        | 描述           |
| ------------- | -------- | ----------- | -------------- |
| id            | TEXT     | PRIMARY KEY | 会话唯一标识符 |
| connection_id | TEXT     | NOT NULL    | 关联的连接 ID  |
| status        | TEXT     | NOT NULL    | 会话状态       |
| terminal_id   | TEXT     |             | 关联的终端 ID  |
| created_at    | INTEGER  | NOT NULL    | 创建时间戳     |
| last_active   | INTEGER  | NOT NULL    | 最后活跃时间戳 |

#### 6.5.3 日志表 (logs)

| 字段名     | 数据类型 | 约束                      | 描述                  |
| ---------- | -------- | ------------------------- | --------------------- |
| id         | INTEGER  | PRIMARY KEY AUTOINCREMENT | 日志唯一标识符        |
| session_id | TEXT     |                           | 关联的会话 ID         |
| level      | TEXT     | NOT NULL                  | 日志级别              |
| message    | TEXT     | NOT NULL                  | 日志消息              |
| details    | TEXT     |                           | 日志详情（JSON 格式） |
| timestamp  | INTEGER  | NOT NULL                  | 日志时间戳            |

## 7. 开发规范

### 7.1 代码风格

#### 7.1.1 Rust 代码规范

- 使用 `rustfmt` 进行代码格式化
- 遵循 Rust 官方风格指南
- 使用 `clippy` 进行代码检查
- 函数和变量命名使用 snake_case
- 类型和 trait 命名使用 CamelCase
- 模块命名使用 snake_case

#### 7.1.2 JavaScript/TypeScript 代码规范

- 使用 `prettier` 进行代码格式化
- 使用 `eslint` 进行代码检查
- 遵循 ESLint 推荐规则
- 函数和变量命名使用 camelCase
- 类命名使用 PascalCase
- 组件命名使用 PascalCase

#### 7.1.3 CSS 代码规范

- 使用 Tailwind CSS 进行样式开发
- 遵循 Tailwind CSS 最佳实践
- 自定义样式使用 BEM 命名规范
- 避免使用内联样式

### 7.2 命名约定

#### 7.2.1 文件命名

- Rust 文件：使用 snake_case
- Svelte 组件：使用 PascalCase
- JavaScript/TypeScript 文件：使用 camelCase

#### 7.2.2 变量命名

- 常量：使用 UPPER_CASE_WITH_UNDERSCORES
- 普通变量：使用 camelCase
- 私有变量：使用 \_camelCase（前端）或 snake_case（后端）

### 7.3 版本控制

- 使用 Git 进行版本控制
- 遵循 Git Flow 工作流
- 主分支：main（生产环境）
- 开发分支：develop
- 特性分支：feature/xxx
- 修复分支：fix/xxx
- 发布分支：release/xxx
- 标签格式：vX.Y.Z

### 7.4 提交信息规范

```
<type>(<scope>): <subject>

<body>

<footer>
```

**类型**：

- feat: 新功能
- fix: 修复 bug
- docs: 文档更新
- style: 代码风格更改
- refactor: 代码重构
- test: 测试更新
- chore: 构建或工具更改

**范围**：

- 模块名称或功能区域

**示例**：

```
feat(connection): add connection grouping feature

Add ability to create and manage connection groups

Closes #123
```

## 8. 环境配置

### 8.1 开发环境要求

| 环境      | 版本  | 备注               |
| --------- | ----- | ------------------ |
| Node.js   | 18.0+ | 前端开发           |
| Rust      | 1.70+ | 后端开发           |
| Cargo     | 1.70+ | Rust 包管理器      |
| Tauri CLI | 2.0+  | Tauri 应用开发工具 |
| Git       | 2.0+  | 版本控制           |

### 8.2 开发环境设置

#### 8.2.1 安装 Rust

```bash
# Linux/macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows
# 下载并安装 https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe
```

#### 8.2.2 安装 Tauri CLI

```bash
cargo install tauri-cli@2.0
```

#### 8.2.3 安装 Node.js

```bash
# 使用 nvm 安装（推荐）
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.3/install.sh | bash
nvm install 18
nvm use 18

# 或直接下载安装包
# https://nodejs.org/en/download/
```

#### 8.2.4 克隆仓库

```bash
git clone https://github.com/your-org/ssh-remote-manager.git
cd ssh-remote-manager
```

#### 8.2.5 安装依赖

```bash
# 安装前端依赖
npm install

# 安装 Rust 依赖
cargo build
```

### 8.3 开发命令

| 命令             | 描述               |
| ---------------- | ------------------ |
| `npm run dev`    | 启动开发服务器     |
| `npm run build`  | 构建生产版本       |
| `cargo test`     | 运行 Rust 测试     |
| `npm test`       | 运行前端测试       |
| `cargo clippy`   | 运行 Rust 代码检查 |
| `npm run lint`   | 运行前端代码检查   |
| `npm run format` | 格式化前端代码     |
| `cargo fmt`      | 格式化 Rust 代码   |

## 9. 测试策略

### 9.1 测试类型

#### 9.1.1 单元测试

- 测试范围：单个函数、模块或组件
- 覆盖率目标：80%+
- 工具：
  - Rust：内置测试框架
  - 前端：Jest + Testing Library

#### 9.1.2 集成测试

- 测试范围：模块间交互、前后端集成
- 覆盖率目标：60%+
- 工具：
  - Rust：内置集成测试
  - 前端：Cypress 或 Playwright

#### 9.1.3 端到端测试

- 测试范围：完整用户流程
- 关键流程测试：
  - SSH 连接建立
  - 终端操作
  - 文件传输
  - 会话管理
- 工具：Playwright

### 9.2 测试流程

1. **开发阶段**：
   - 编写单元测试
   - 运行本地测试
   - 确保测试通过

2. **提交阶段**：
   - 代码检查（clippy、eslint）
   - 单元测试运行
   - 构建检查

3. **CI/CD 阶段**：
   - 运行所有测试
   - 生成测试报告
   - 代码覆盖率检查

4. **发布前**：
   - 端到端测试
   - 跨平台测试
   - 性能测试

### 9.3 测试环境

- 测试 SSH 服务器：使用 Docker 容器运行 OpenSSH 服务器
- 测试数据库：内存中的 SQLite 数据库
- 测试文件系统：临时目录

## 10. 部署流程

### 10.1 构建流程

1. **代码准备**：
   - 切换到 release 分支
   - 更新版本号
   - 运行最终测试

2. **构建应用**：

   ```bash
   # 构建所有平台
   npm run build:all

   # 或构建特定平台
   npm run build:windows
   npm run build:macos
   npm run build:linux
   ```

3. **构建产物**：
   - Windows：MSI 安装包
   - macOS：DMG 磁盘镜像
   - Linux：DEB、RPM 包和 AppImage

### 10.2 发布流程

1. **代码签名**：
   - Windows：使用 Authenticode 证书签名
   - macOS：使用 Apple Developer ID 签名并公证
   - Linux：使用 GPG 签名

2. **发布准备**：
   - 更新 CHANGELOG.md
   - 创建发布标签
   - 准备发布说明

3. **分发渠道**：
   - 官方网站下载
   - GitHub Releases
   - 应用商店（Windows Store、macOS App Store、Snap Store）
   - 包管理器（Homebrew、Chocolatey）

### 10.3 版本更新流程

1. **检查更新**：应用启动时检查更新
2. **通知用户**：显示更新通知
3. **下载更新**：后台下载更新包
4. **安装更新**：
   - Windows：自动运行安装程序
   - macOS：自动挂载 DMG 并提示用户安装
   - Linux：提示用户使用包管理器更新

## 11. 附录

### 11.1 常见问题

#### 11.1.1 连接失败

**可能原因**：

- 主机名或 IP 地址错误
- 端口号错误
- 用户名或密码错误
- 防火墙阻止连接
- SSH 服务器未运行

**解决方案**：

- 检查连接配置
- 测试网络连接
- 检查 SSH 服务器状态
- 检查防火墙设置

#### 11.1.2 终端显示异常

**可能原因**：

- 终端大小不匹配
- 终端类型设置错误
- 字符编码问题

**解决方案**：

- 调整终端大小
- 检查终端类型设置
- 确保使用 UTF-8 编码

#### 11.1.3 文件传输失败

**可能原因**：

- 权限不足
- 磁盘空间不足
- 网络连接中断
- 文件路径错误

**解决方案**：

- 检查文件权限
- 检查磁盘空间
- 确保网络连接稳定
- 检查文件路径

### 11.2 故障排除

#### 11.2.1 查看日志

**Rust 日志**：

- 日志文件位置：
  - Windows：`%APPDATA%/ssh-remote-manager/logs/`
  - macOS：`~/Library/Logs/ssh-remote-manager/`
  - Linux：`~/.local/share/ssh-remote-manager/logs/`

**前端日志**：

- 在应用中打开开发者工具（Ctrl+Shift+I 或 Cmd+Option+I）
- 查看控制台输出

#### 11.2.2 调试模式

启动应用时添加 `--debug` 参数以启用调试模式：

```bash
# Windows
ssh-remote-manager.exe --debug

# macOS
open -a "SSH Remote Manager" --args --debug

# Linux
./ssh-remote-manager --debug
```

### 11.3 开发工具链

| 工具       | 用途            | 命令                  |
| ---------- | --------------- | --------------------- |
| Cargo      | Rust 包管理     | `cargo <command>`     |
| Tauri CLI  | Tauri 应用开发  | `tauri <command>`     |
| npm        | 前端包管理      | `npm <command>`       |
| rustfmt    | Rust 代码格式化 | `cargo fmt`           |
| clippy     | Rust 代码检查   | `cargo clippy`        |
| prettier   | 前端代码格式化  | `npm run format`      |
| eslint     | 前端代码检查    | `npm run lint`        |
| Jest       | 前端单元测试    | `npm test`            |
| Playwright | 端到端测试      | `npx playwright test` |

### 11.4 资源链接

- [Rust 官方文档](https://doc.rust-lang.org/)
- [Tauri 官方文档](https://tauri.app/)
- [Svelte 官方文档](https://svelte.dev/)
- [russh 文档](https://docs.rs/russh/)
- [xterm.js 文档](https://xtermjs.org/docs/)
- [SQLite 文档](https://www.sqlite.org/docs.html)

---

**文档结束**
