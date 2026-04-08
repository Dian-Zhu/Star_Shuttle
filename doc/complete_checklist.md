# Star Shuttle 完整清单（功能 / 安全 / 工程质量）

本清单用于把“需求文档 vs 当前实现”落到可执行的任务项。每一项都给出：状态、落点位置（代码/模块）、建议优先级、验收方式。

状态约定：
- ✅ 已实现：端到端可用，且有基本验证
- 🟡 部分实现：有结构/雏形，但缺关键链路或不可稳定使用
- ❌ 未实现：缺失或仅存在文档/空壳

优先级约定：
- P0：阻断迭代/基础不可用/明显安全缺口
- P1：核心能力缺失或高风险问题
- P2：专业能力增强与体验提升
- P3：锦上添花/长期演进

---

## 1. 工程结构与关键链路清单

### 1.1 前端（Svelte 5 + Vite）
- ✅ 基础启动链路：`src/main.ts` -> `src/App.svelte` -> `src/components/Layout.svelte`
  - 位置：`src/main.ts`，`src/App.svelte`，`src/components/Layout.svelte`
  - 验收：`npm run build` 成功
- ✅ 状态管理：`src/lib/store.ts`
  - 现状：连接/终端/设置/锁屏等 store 基本齐全
  - 验收：`npm run check` 通过
- ✅ IPC 调用：`@tauri-apps/api/core` 的 `invoke`
  - 位置：`src/lib/*Service.ts`、`src/components/*`
  - 现状：invoke 参数命名已统一为 snake_case，避免与后端不一致

### 1.2 后端（Tauri 2 + Rust）
- ✅ 命令入口与注册：`src-tauri/src/lib.rs`
  - 位置：`src-tauri/src/lib.rs`（`tauri::generate_handler!`）
  - 验收：`cargo test` 成功
- 🟡 连接与终端：`src-tauri/src/modules/connection/mod.rs`
  - 现状：连接、会话、terminal event 通道已成型
  - 验收：连接配置可持久化（见 3.1）
- ✅ SFTP：`src-tauri/src/modules/sftp/mod.rs`
  - 现状：ls/read/write/mkdir/rm/rmdir/rename 已有命令
  - 缺口：缺少 owner/group/权限修改等（见 5.3）
- ✅ DB：`src-tauri/src/modules/db/mod.rs`
  - 现状：settings、command_snippets、audit_events 表 + CRUD
  - 缺口：connection_profiles 表未接入（见 3.1）

---

## 2. 工程质量（P0）

### 2.1 类型体系与 TS 编译健康度
- ✅ `src/types/index.ts` 修复（移除重复定义，恢复可解析）
  - 位置：`src/types/index.ts`
  - 优先级：P0
  - 验收：`npm run check` 通过

### 2.2 Lint / Typecheck / Build 基线
- ✅ `npm run lint` 通过（0 errors）
  - 位置：`.eslintrc.cjs`、`src/components/*`、`src/lib/*`、`src/types/*`
  - 优先级：P0
  - 验收：`npm run lint` 通过（0 errors）
- ✅ `npm run check` 通过
  - 优先级：P0
  - 验收：`npm run check` 通过
- ✅ `npm run build` 可通过（存在 a11y 警告）
  - 建议：把 a11y 警告逐步清零或明确降级策略

### 2.3 建议加的最小工程护栏
- 🟡 统一脚本顺序与门禁
  - 建议动作：在 CI 或 pre-commit 里固定执行 `lint -> check -> build -> cargo test`
  - 验收：同一套命令在本机与 CI 一致

---

## 3. 连接管理（P0/P1）

### 3.1 连接配置持久化（P0）
- ✅ 连接配置已持久化（SQLite settings 存 JSON，重启可恢复）
  - 位置：`src-tauri/src/modules/connection/mod.rs`（`DefaultConnectionManager.connections`）
  - 同时存在未接入的 DB 表：`connection_profiles`（`src-tauri/src/modules/db/mod.rs`）
  - 优先级：P0
  - 说明：
    - 连接配置通过 `DatabaseManager.settings` 的 `connection_configs` 键存储
    - 持久化前会对敏感字段做去敏（password/passphrase 不落盘）
  - 验收：
    - 新建连接 -> 重启应用 -> 仍可见
    - 删除连接 -> 重启后不出现
    - 导入/导出能在重启后保持一致

### 3.2 连接分组/标签/搜索（P1）
- ✅ 标签：表单支持编辑 tags，后端结构为 `tags: Vec<String>` 并可持久化
  - 位置：`src/components/ConnectionModal.svelte`、`src/lib/connectionService.ts`、`src-tauri/src/modules/connection/mod.rs`
- ✅ 分组：支持创建/重命名/删除分组并将连接归类（`group_id` 正常读写）
  - 位置：`src/components/Sidebar.svelte`、`src/components/ConnectionModal.svelte`、`src/lib/connectionService.ts`
  - 优先级：P1
  - 验收：可创建/重命名/删除分组并将连接归类
- ✅ 编辑连接：支持编辑已有连接配置，且不破坏已保存的凭证
  - 位置：`src/components/Sidebar.svelte`、`src/components/ConnectionModal.svelte`、`src/lib/connectionService.ts`、`src-tauri/src/modules/connection/mod.rs`
  - 验收：
    - 可从侧边栏进入编辑并保存
    - 已勾选“保存密码/保存跳板密码”的连接，编辑后仍可连接（无需再次输入密码）

### 3.3 连接历史（P2）
- ✅ 前端本地历史：`localStorage`（最多 50 条）
  - 位置：`src/lib/store.ts`
  - 缺口：后端未记录“所有连接尝试”（文档期望）

---

## 4. 认证与凭证安全（P0/P1）

### 4.1 Keyring 凭证存储（P0）
- ✅ keyring 凭证存储已实现（按 connection_id 存取）
  - 位置：`src-tauri/src/modules/credential/mod.rs`
  - 优先级：P0
  - 验收：
    - ✅ 勾选保存密码 -> 重启仍可连接（不需要再次输入）
    - ✅ 不勾选 -> 重启后要求重新输入

### 4.2 导入/导出安全（P0/P1）
- ✅ 默认导出已剥离敏感字段（可选包含敏感信息并强提示）
  - 位置：`src/lib/importExportService.ts`
  - 优先级：P0（至少要提示与可选去敏）
  - 验收：导出文件默认不含密码；选择包含时给出强提示

### 4.3 证书认证 / MFA（P1/P2）
- 🟡 证书认证后端实现不完整（当前依赖栈不支持 OpenSSH cert 公钥解析）
  - 位置：`src-tauri/src/modules/connection/ssh_impl.rs`
  - 说明：`russh-keys 0.40.x` 的 `PublicKey`/`parse_public_key` 不支持 `*-cert-v01@openssh.com`，当前会明确拒绝该类型
  - 优先级：P1
  - 验收：证书方式可连接真实服务器
- ✅ MFA/keyboard-interactive UX（多轮挑战）
  - 位置：`src-tauri/src/modules/connection/mod.rs`、`src-tauri/src/modules/connection/ssh_impl.rs`、`src/components/Layout.svelte`
  - 优先级：P2
  - 验收：弹窗/提示清晰，支持多轮挑战

---

## 5. SSH 安全与网络能力（P1）

### 5.1 主机密钥验证（Known Hosts）（P0/P1）
- ✅ known_hosts 已解析并按标准格式保存，首次连接会提示并可选择保存
  - 位置：`src-tauri/src/modules/connection/known_hosts.rs`
  - 优先级：P0/P1
  - 建议动作：首次连接弹窗展示 fingerprint 并允许 accept/reject/save
  - 验收：首次连接提示；再次连接不提示；变更指纹会阻断并提示风险

### 5.2 代理 / 跳板 / 端口转发（P1）
- ✅ JumpHost 双跳与 -D 动态端口转发已打通（最小可用链路）
  - 位置：`src-tauri/src/modules/connection/mod.rs`（JumpHost 连接编排）、`src-tauri/src/modules/connection/ssh_impl.rs`（本地 SOCKS5 + direct-tcpip）
  - 优先级：P1
  - 验收：
    - JumpHost：通过跳板机连接目标主机，且目标主机 known_hosts 校验生效
    - -D：配置 socks_proxy_port 后，会在本机 127.0.0.1:<port> 启动 SOCKS5

### 5.3 文件传输（SFTP/SCP）（P1/P2）
- ✅ SFTP 基础操作已实现（ls/read/write/mkdir/rm/rename）
  - 位置：`src-tauri/src/modules/sftp/mod.rs`
- ✅ SCP 回退已实现（读/写失败时可改走 SCP）
  - 位置：`src-tauri/src/modules/sftp/mod.rs`，`src/lib/sftpService.ts`，`src/components/file-transfer/FileExplorer.svelte`，`src/lib/transferQueueService.ts`
  - 优先级：P2
- ✅ 文件权限/owner/group 已返回（无法解析时回退 UID/GID）
  - 位置：`src-tauri/src/modules/sftp/mod.rs`，`src/lib/sftpService.ts`
  - 优先级：P2

---

## 6. 终端体验（P1/P2）

### 6.1 终端数据链路与会话
- ✅ xterm.js + tauri events 链路已打通
  - 位置：`src/lib/terminalService.ts`，后端 connection 模块 terminal 相关命令
  - 验收：连接后可输入/输出，resize 生效

### 6.2 自动重连（P2）
- ✅ 自动重连已完善（有限次数 + 指数退避 + 终端/Toast 状态）
  - 位置：`src/lib/terminalService.ts`

### 6.3 终端增强项（P2/P3）
- ✅ 多选 tab 广播输入（最小可用链路）
  - 位置：`src/components/TerminalManager.svelte`，`src/lib/terminalService.ts`，`src/lib/store.ts`
  - 验收：开启“广播”后，Ctrl/⌘ 点击多选 Tab，输入会同步发送到所有选中会话
- ✅ 搜索：支持快捷键与统一面板
  - 位置：`src/components/TerminalView.svelte`
  - 验收：Ctrl/⌘+F 打开/关闭搜索栏；Enter/Shift+Enter 上下跳转；Esc 关闭并回到终端
- ✅ WebGL 加速：已集成 xterm WebGL renderer（自动回退）
  - 位置：`src/lib/terminalService.ts`

---

## 7. 审计与日志（P1/P2）

### 7.1 审计事件闭环（P1）
- ✅ 输入高危命令会二次确认，并记录到 DB
  - 位置：`src/lib/auditService.ts`、`src/lib/terminalService.ts`
  - 优先级：P1
  - 验收：输入高危命令时弹出警告/二次确认，并记录到 DB

### 7.2 日志策略一致性（P2）
- ✅ 后端有自定义日志器与轮转
  - 位置：`src-tauri/src/modules/logging/mod.rs`
- ✅ 前端已提供“查看/导出日志”入口
  - 位置：`src/components/AdvancedModal.svelte`

---

## 8. 设置与锁屏（P2）

### 8.1 设置面板完整性
- ✅ settings store 与 UI/行为闭环
  - 位置：`src/lib/store.ts`、`src/components/SettingsModal.svelte`、`src/components/Layout.svelte`、`src/components/TerminalView.svelte`
  - 验收：主题/快捷键/侧边栏/安全设置与实际行为一致；快捷键冲突会提示并阻止写入

### 8.2 App Lock（P2）
- ✅ 基础能力：设置/校验/开启状态
  - 位置：后端命令 `set_app_lock/verify_app_lock/is_app_lock_enabled/remove_app_lock`
  - 前端联动：`Layout.svelte` 自动锁
  - 缺口：更细的安全策略（重试次数、锁定时长、敏感信息遮罩）

---

## 9. 文档一致性（P2/P3）

### 9.1 文档与代码偏差修正
- 🟡 `software_architecture.md` / `development_documentation.md` 中有“模块与文件结构”与真实仓库不一致
  - 优先级：P3（不阻断交付，但会误导新人/后续维护）
  - 验收：文档中的路径、版本、模块边界与仓库一致

### 9.2 需求追踪矩阵（建议）
- 🟡 建议把 `functional_specification.md` 的条目编号化，建立“需求 -> 代码 -> 测试”的可追踪表
  - 优先级：P2

---

## 10. AI Agent 重构专项（P1/P2）

### 10.1 Agent 架构重构（P1）
- ✅ Agent 后端已完成分层重构
  - 位置：`src-tauri/src/modules/ai/agent.rs`、`planner.rs`、`orchestrator.rs`、`agent_store.rs`、`agent_types.rs`、`tools/`
  - 现状：
    - 已由 `Planner + Orchestrator + Tool Registry + Persisted History` 四层替代旧单循环实现
    - 任务/步骤/事件已落库
    - 新 Tauri 接口 `ai_agent_*` 已接入前端
  - 验收：
    - `cargo test` 通过
    - `npm test` 通过
    - `npm run check` 通过

### 10.2 Agent 前端快照与事件流（P1）
- ✅ 当前任务与历史查看首版已落地
  - 位置：`src/lib/aiAgentService.ts`、`src/components/ai/AiAgentPanel.svelte`
  - 现状：
    - 当前任务使用快照 + 事件流渲染
    - 最近任务历史可查看
    - 首版不支持应用重启后的继续执行

### 10.3 下一任务（P2）
- 🟡 补齐 Agent 命令层接口测试与历史回放细节
  - 优先级：P2
  - 建议动作：
    - 增加 `ai_agent_start / confirm / cancel / get_task / get_task_events` 的更高层测试
    - 增强历史事件展示，明确失败、拒绝、确认超时等原因
  - 验收：
    - 关键终态具备更高层测试覆盖
    - 历史面板能清晰区分 `completed / failed / cancelled / rejected / timed_out`

---

## 11. 推荐推进顺序（最短路径）

1) P0：修复类型体系与 lint/check 基线（先让工程可持续迭代）
2) P0：连接配置持久化 + 凭证 keyring 落地（安全与数据不丢）
3) P1：known_hosts 校验闭环（安全底线）
4) P1：审计链路闭环（企业级能力的骨架）
5) P2：文件传输真实断点续传 + 队列 UI
6) P2：代理/跳板/端口转发逐项打通并补测试
