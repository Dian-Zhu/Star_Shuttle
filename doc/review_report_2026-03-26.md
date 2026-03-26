# Star Shuttle 工程审查报告

审查日期：2026-03-26

本报告由 4 个独立 Agent 并行审查后汇总而成，审查点分别为：

- 安全漏洞
- 代码质量
- 潜在 Bug
- 竞态条件

我另外做了本地交叉校验与命令验证，避免直接照抄子结论。

## 修复闭环状态

截至本次收尾，这份报告中的高风险与中风险可修复项已经完成修复，状态如下：

- 安全漏洞：已修复
  - 禁止明文凭据 fallback
  - 导出脱敏覆盖代理与跳板认证字段
  - 删除后端任意路径写命令，下载改走 `plugin-fs.writeFile`
  - 应用锁变更/删除下沉到后端校验旧密码
  - 发布构建移除 DevTools 暴露并恢复 CSP
- 潜在 Bug：已修复
  - 传输队列停滞与重复完成
  - 分块下载失败后 SCP 回退写坏文件
  - 文件面板跨 session 长操作错写
  - 重连后旧 session 未退休
  - best-effort 关闭逻辑错误
- 竞态条件：已修复主要实锤项
  - `disconnect` 现在会联动关闭 terminal 并清理 SFTP session
  - SFTP 取 session 前会校验连接状态
  - SSH terminal 改为先启动成功再登记，避免脏状态
  - 前端关闭/重连路径增加统一状态清理
- 代码质量：已完成本轮落地项
  - 前端静态检查恢复为全绿
  - `connectionService` 收敛 DTO 类型边界
  - 终端实例注册收敛到单一 owner，去掉组件侧重复注册
  - 新增前端回归测试基础设施与关键测试

## 最终验证结果

- `cargo check`：通过
- `cargo test`：通过，12 个 Rust 单元测试全部通过
- `npm run check`：通过，0 errors / 0 warnings
- `npm test`：通过，2 个测试文件、4 个测试全部通过
- `npm run build`：通过
  - 已移除动态导入告警
  - 已将主前端包拆分为 `index` / `vendor-xterm` / `vendor-tauri` / `vendor-svelte`

## 高优先级结论

当前工程最需要优先处理的不是单一问题，而是一组会彼此放大的缺陷：

1. 明文凭据落库与代理密码导出会直接造成敏感信息泄露。
2. 自定义 `save_file_to_local` 绕过 capability 约束，后端暴露了任意本地路径写入面。
3. 传输队列、下载回退、重连与文件面板存在多处高危行为 Bug，已经会影响数据正确性。
4. 后端 session 生命周期拆散在 `sessions`、`terminals`、`ssh_connections`、`SftpManager.sessions` 几套状态里，没有统一 shutdown 协调器，竞态问题明显。

## 安全漏洞

### 1. 明文凭据会落入 SQLite，代理密码还会默认进入持久化配置与导出文件

- 严重级别：高
- 位置：
  - `src-tauri/src/modules/credential/mod.rs:38-48`
  - `src-tauri/src/modules/credential/mod.rs:72-99`
  - `src-tauri/src/modules/connection/mod.rs:684-708`
  - `src-tauri/src/modules/connection/mod.rs:723-727`
  - `src-tauri/src/modules/connection/mod.rs:1389-1392`
  - `src/lib/importExportService.ts:10-47`
  - `src/lib/importExportService.ts:60-78`
- 证据：
  - `CredentialManager::fallback_save` 在 keyring 不可用时直接调用 `db.save_setting(..., password)`。
  - `sanitize_proxy_type` 对 `Socks5`/`Http` 代理密码没有做脱敏，和主 SSH 密码的处理方式不一致。
  - `sanitizeConnectionForExport` 只处理 `auth_method`，没有清理 `proxy_type` 里的密码字段。
- 影响：
  - keyring 失效时，SSH 密码/口令会明文落入 `app.db`。
  - 用户即使不勾选“导出敏感信息”，仍可能把代理密码导出到 JSON。
- 建议：
  - 禁止明文 fallback；无法使用安全存储时，直接拒绝“保存密码”。
  - 持久化与导出都改为“敏感字段白名单剔除”。
  - 代理认证字段按主认证字段同等处理。

### 2. `save_file_to_local` 直接暴露任意本地路径写入

- 严重级别：高
- 位置：
  - `src-tauri/src/lib.rs:274-281`
  - `src-tauri/src/lib.rs:527-579`
  - `src-tauri/capabilities/default.json:15-81`
- 证据：
  - 命令实现直接 `File::create(&path)` 并 `write_all`，没有路径白名单、目录边界校验或来源校验。
  - capability 已经对插件文件系统做了路径限制，但该自定义 Rust command 完全绕过了这层限制。
- 影响：
  - 一旦 renderer 被注入任意脚本，就可以按当前用户权限覆盖任意可写路径文件。
- 建议：
  - 删除该命令，统一走 `tauri-plugin-fs`。
  - 如果必须保留，后端必须校验 path 落在允许目录内，并绑定到用户单次选择的受信路径。

### 3. 应用锁的关键校验只在前端，后端允许直接覆盖或删除

- 严重级别：中
- 位置：
  - `src-tauri/src/lib.rs:16-25`
  - `src-tauri/src/lib.rs:47-52`
  - `src/components/SettingsModal.svelte:320-364`
- 证据：
  - `set_app_lock` 会无条件覆盖现有 hash。
  - `remove_app_lock` 会无条件删除现有 hash。
  - “必须先验证旧密码”只在前端界面逻辑里体现，没有下沉到 Rust command。
- 影响：
  - 任何能调用 Tauri command 的前端代码都可以跳过旧密码验证，直接改锁或删锁。
- 建议：
  - 后端拆成 `create_lock`、`change_lock(old,new)`、`remove_lock(old)` 三个显式接口。
  - 将旧密码校验、失败节流和错误反馈收敛到后端。

### 4. 删除连接配置时未清理 jump host 凭据

- 严重级别：中
- 位置：
  - `src-tauri/src/modules/connection/mod.rs:920-999`
  - `src-tauri/src/modules/connection/mod.rs:1400-1410`
- 证据：
  - 保存逻辑会单独存储 `jump_password` / `jump_passphrase`。
  - 删除逻辑只清理主连接的 `password` / `passphrase`，没有清理 jump host 对应条目。
- 影响：
  - 用户删除连接后，jump host 凭据仍可能留在 keyring 或 DB fallback 里。
- 建议：
  - 删除配置时同步清理 `jump_password` 和 `jump_passphrase`。
  - 为凭据生命周期补回归测试。

### 5. 生产构建保留 DevTools，且 CSP 被完全关闭

- 严重级别：中
- 位置：
  - `src-tauri/Cargo.toml:24`
  - `src-tauri/tauri.conf.json:24-26`
  - `src-tauri/src/lib.rs:438-445`
- 证据：
  - `tauri` 依赖启用了 `devtools` feature。
  - 注册了 `toggle_devtools` command。
  - `security.csp` 为 `null`。
- 影响：
  - 这本身不是独立远程漏洞，但会显著降低 renderer 被攻陷后的利用门槛。
- 建议：
  - 发布版关闭 devtools feature，并移除对应 command/UI 入口。
  - 配置最小可用 CSP，而不是 `null`。

## 代码质量

### 1. 静态检查已经失效，基础质量门没有守住

- 严重级别：中
- 位置：
  - `src/lib/store.ts:940`
  - `src/components/terminal/TerminalPane.svelte:3`
- 证据：
  - `npm run check` 当前直接失败。
- 影响：
  - 说明前端最基本的类型/静态检查没有被当作必过门禁。
- 建议：
  - 先恢复 `npm run check` 为绿色，再继续堆叠功能。
  - 将 `npm run check` 和 `cargo test` 纳入 CI 必过项。

### 2. 终端与全局状态已经膨胀为超大单体模块

- 严重级别：中
- 位置：
  - `src/lib/terminalService.ts`，1758 行
  - `src/lib/store.ts`，1237 行
  - `src/lib/terminalService.ts:578`
  - `src/lib/terminalService.ts:768`
- 证据：
  - `terminalService.ts` 同时负责终端初始化、监听器、输入输出节流、重连、恢复、清理和 UI 消息。
  - `initTerminal` 与 `initDetachedTerminal` 存在大面积重复。
- 影响：
  - 会让终端相关改动难以局部推理，回归风险高。
- 建议：
  - 将终端逻辑至少拆成生命周期、IO、重连、UI 绑定四层。
  - 收敛重复初始化路径。

### 3. 连接配置转换依赖 `any` 和手工分支，类型边界不清晰

- 严重级别：中
- 位置：
  - `src/lib/connectionService.ts:56`
  - `src/lib/connectionService.ts:81`
- 证据：
  - `createBackendConfig(connectionData: any)` 与 `toBackendConnectionConfig(...)` 手工拼装多协议、多认证、多代理结构。
- 影响：
  - 前后端协议一旦漂移，问题只会在运行时暴露。
- 建议：
  - 定义明确的表单 DTO、前端域模型、后端 IPC DTO。
  - 用判别联合替代 `any`，并为转换逻辑补测试。

### 4. 终端实例所有权分散在多个模块，生命周期职责重叠

- 严重级别：中
- 位置：
  - `src/components/TerminalView.svelte:27`
  - `src/lib/terminalService.ts:631`
  - `src/lib/terminalPool.ts:33`
  - `src/lib/terminalInstance.ts:514`
- 证据：
  - 组件层、`terminalService`、`terminalPool`、`terminalInstance` 都参与了实例注册/接管/销毁。
- 影响：
  - 分屏、重连、重复销毁类问题难以定位，行为边界不清晰。
- 建议：
  - 明确单一 owner。更合理的做法是让 `terminalPool` 统一管理实例生命周期。

### 5. 自动化测试覆盖偏后端，前端关键路径缺少回归保护

- 严重级别：中
- 位置：
  - `src-tauri/src/modules/connection/mod.rs:2300`
  - `src-tauri/src/modules/connection/tracking.rs:124`
- 证据：
  - Rust 侧只有 12 个测试，且仓库没有前端 `*.test.*` / `*.spec.*` 文件。
- 影响：
  - 分屏、重连、传输队列、会话恢复这些复杂前端路径基本没有自动化保护。
- 建议：
  - 优先给 `terminalService`、`transferQueueService`、`connectionService` 加 Vitest。
  - 对重连、分屏、文件传输补端到端测试。

## 潜在 Bug

### 1. 传输队列在首批任务后会停滞，上传路径还会重复“完成”

- 严重级别：高
- 位置：
  - `src/lib/transferQueueService.ts:153-180`
  - `src/lib/transferQueueService.ts:285`
  - `src/lib/transferQueueService.ts:364`
- 证据：
  - `processQueue()` 只在新增和恢复时触发。
  - `completeTransfer()`、`failTransfer()`、取消路径不会再推进队列。
  - 上传分支在 `:285` 和函数尾部 `:364` 各调用了一次 `completeTransfer(id)`。
- 影响：
  - 超过并发上限后的任务可能永久卡在 `pending`。
  - 已完成任务会出现重复完成、重复清理或状态抖动。
- 建议：
  - 将队列推进收敛为统一状态机，并保证 `completeTransfer` 幂等且只调用一次。

### 2. 分块下载失败后回退到 SCP，会在旧偏移上继续写整文件

- 严重级别：高
- 位置：
  - `src/lib/transferQueueService.ts:300-349`
- 证据：
  - 分块下载已写入一部分内容后，如果 `readChunk()` 失败，会进入 `scpDownload()` 回退。
  - 此时 `fileHandle` 可能仍停在旧偏移，代码直接把整文件再次 `writeChunk()` 到当前句柄。
- 影响：
  - 本地下载文件会出现重复内容、尾部污染或整体损坏。
- 建议：
  - 回退前重新以截断模式打开文件，或显式 `seek(0)` 后覆盖写入。

### 3. 重连成功后旧 session 没有被彻底退休

- 严重级别：高
- 位置：
  - `src/lib/terminalService.ts:926`
  - `src/lib/terminalService.ts:1179`
  - `src/lib/terminalService.ts:1574`
  - `src/lib/terminalService.ts:1684`
  - `src-tauri/src/modules/connection/mod.rs:1310`
- 证据：
  - `reconnectTerminal()` 替换了 UI 里的 `sessionId`，但没有像正常关闭路径那样清理旧输入状态，也没有对旧 session 调用后端 `disconnect`。
- 影响：
  - 可能把缓冲输入发到旧会话，形成幽灵 session，前后端会话状态不一致。
- 建议：
  - 重连前先清理旧 session 的输入/输出状态、定时器和监听器，并调用 `close_terminal + disconnect(oldSessionId)`。

### 4. 文件面板的长操作直接读取可变 `sessionId`，切换主机后可能写到另一台机器

- 严重级别：高
- 位置：
  - `src/components/file-transfer/FileExplorer.svelte:168`
  - `src/components/file-transfer/FileExplorer.svelte:472`
  - `src/components/file-transfer/FileExplorer.svelte:609`
  - `src/components/file-transfer/FileExplorer.svelte:855`
- 证据：
  - 上传、粘贴、保存编辑后文件等流程跨多个 `await` 继续使用组件级 `sessionId`。
  - session 切换时只清了缓存和目录加载，没有取消这些 mutation。
- 影响：
  - 同一操作前半段可能落到主机 A，后半段落到主机 B，属于直接数据错写。
- 建议：
  - 异步操作开始时快照 `targetSessionId`，后续只用快照。
  - session 切换时取消进行中的写操作。

### 5. `closeBackendTerminalsBestEffort()` 成功后仍继续循环调用

- 严重级别：中
- 位置：
  - `src/lib/terminalService.ts:1318`
- 证据：
  - `invoke('close_terminal')` 成功后没有 `return` 或 `break`，循环还会继续。
- 影响：
  - 会产生多余 IPC 和误导性 `Session not found` 错误。
- 建议：
  - 成功后立即退出；如需重试，只在明确可重试错误上进行。

### 6. `sftp_read_chunk()` 忽略 seek 失败，可能返回错误偏移的数据

- 严重级别：中
- 位置：
  - `src-tauri/src/modules/sftp/mod.rs:620-645`
- 证据：
  - 当 `offset > 0` 时，`file.seek(...)` 的错误被直接丢弃：`let _ = file.seek(...)`。
- 影响：
  - 上层以为自己拿到了指定偏移的数据，实际可能从头读，导致预览、断点下载或文本编辑逻辑错误。
- 建议：
  - seek 失败要显式报错，不能静默降级。

## 竞态条件

### 1. `disconnect()` 不会终止已启动的终端任务

- 严重级别：高
- 位置：
  - `src-tauri/src/modules/connection/mod.rs:1310-1334`
  - `src-tauri/src/modules/connection/mod.rs:1834-2043`
  - `src-tauri/src/modules/connection/mod.rs:2126-2143`
  - `src-tauri/src/lib.rs:145-152`
- 证据：
  - `disconnect()` 只移除连接 map 并改 session 状态。
  - 真正的终端任务在 `start_terminal()` 里 `runtime.spawn(...)`，没有取消 token，也不会被 `disconnect()` 主动关闭。
- 影响：
  - UI 已显示“断开连接”，后台仍可能继续发 `terminal-output-*` 或 `session-closed-*`。
- 建议：
  - 在 `disconnect()` 中同步关闭该 session 的 terminal sender，并引入统一的 shutdown 信号。

### 2. SFTP session 缓存与断连清理脱节

- 严重级别：高
- 位置：
  - `src-tauri/src/modules/sftp/mod.rs:27-29`
  - `src-tauri/src/modules/sftp/mod.rs:160-205`
  - `src-tauri/src/modules/sftp/mod.rs:340-520`
  - `src-tauri/src/modules/connection/mod.rs:1310-1334`
- 证据：
  - `SftpManager` 维护独立的 session cache。
  - `disconnect()` 没有通知 `SftpManager` 清理缓存。
  - `get_session()` 命中缓存后不再核对连接管理器里的 session 是否还处于 `Connected`。
- 影响：
  - 断连和后续 `sftp_ls/read/write/rm` 存在明显时序竞争，可能出现“已断开但旧 SFTP 仍能操作或随机失败”。
- 建议：
  - `disconnect()` 联动清理 SFTP cache。
  - 每次取缓存前校验 session 当前状态。

### 3. 终端在 PTY/Shell 真正启动成功前就被登记

- 严重级别：中
- 位置：
  - `src-tauri/src/modules/connection/mod.rs:1834-1868`
  - `src-tauri/src/modules/connection/mod.rs:2045-2058`
  - `src-tauri/src/modules/connection/mod.rs:2063-2148`
- 证据：
  - `start_terminal()` 先把 terminal 写入 `self.terminals`，再异步执行 `request_pty()` 和 `request_shell()`。
  - 启动失败时只发错误事件，没有回滚 `self.terminals` 和 `session.terminal_id`。
- 影响：
  - `send_terminal_data` / `resize_terminal` / `close_terminal` 可能打到一个“逻辑存在、实际未启动成功”的终端。
- 建议：
  - 成功申请 PTY 与 shell 后再登记 terminal，或失败时显式回滚。

### 4. `send_terminal_data()` 与 `close_terminal()` 存在发送顺序竞争

- 严重级别：中
- 位置：
  - `src-tauri/src/lib.rs:285-308`
  - `src-tauri/src/modules/connection/mod.rs:1989-2004`
  - `src-tauri/src/modules/connection/mod.rs:2126-2143`
- 证据：
  - `send_terminal_data()` 先克隆 sender，释放锁后再异步 `send(Data)`。
  - `close_terminal()` 会移除 terminal 并 `blocking_send(Close)`。
  - 两者之间没有顺序屏障，也没有 `closing` 状态位。
- 影响：
  - 用户关闭前最后几次输入可能仍被送到远端，也可能被丢弃，行为依赖时序。
- 建议：
  - 关闭开始后拒绝新输入，或将输入与关闭统一串行化到单一 actor。

## 建议的修复顺序

1. 先处理安全面：去掉明文凭据 fallback、代理密码脱敏、删除任意路径写入 command。
2. 再处理数据正确性：修复传输队列停滞、下载回退损坏、文件面板跨 session 错写。
3. 然后收敛 session 生命周期：统一 `disconnect` / `close_terminal` / `SFTP cleanup` / `reconnect` 的关闭路径。
4. 最后补质量门：修复 `npm run check`、建立前端测试与 CI。
