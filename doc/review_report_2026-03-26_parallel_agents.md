# 工程审查报告（并行 Agent 汇总）

日期：2026-03-26

本次审查将以下 4 个审查点拆分给独立 Agent 并行执行，并在主线程做了交叉复核：

- 安全漏洞
- 代码质量
- 潜在 Bug
- 竞态条件

补充验证结果：

- `npm test`：通过
- `npm run check`：通过
- `cargo test --manifest-path src-tauri/Cargo.toml`：通过

## 安全漏洞

### 1. High: 应用锁只在前端做遮罩，后端命令没有解锁校验

- 证据：
  - `src/components/Layout.svelte:173`
  - `src-tauri/src/lib.rs:17`
  - `src-tauri/src/lib.rs:98`
  - `src-tauri/src/lib.rs:207`
  - `src-tauri/src/lib.rs:368`
  - `src-tauri/src/lib.rs:451`
- 结论：
  - 当前应用锁通过前端 `isLocked` 控制 UI 遮罩，但后端 Tauri command 并没有统一校验“是否已解锁”。
  - 只要 WebView 内仍能调用 `invoke`，理论上就能在锁屏状态下访问连接配置、执行命令、读取日志等敏感能力。
- 影响：
  - 应用锁更像 UI 级阻挡，而不是可信的权限边界。

### 2. High: RDP 启动会在系统临时目录创建可预测文件名的 `.rdp` 文件

- 证据：
  - `src-tauri/src/lib.rs:109`
  - `src-tauri/src/lib.rs:120`
  - `src-tauri/src/lib.rs:137`
- 结论：
  - `write_temp_rdp_file` 用 `host + port` 组合出固定文件名，并直接写到系统临时目录。
  - 文件包含目标主机、端口、用户名，且没有显式权限收紧与使用后清理。
- 影响：
  - 会残留敏感连接元数据。
  - 在多用户环境下存在本地信息泄露面，也有被预置同名路径干扰的风险。

### 3. Medium: Keyboard-Interactive 请求在 `emit` 失败时会留下悬空状态

- 证据：
  - `src-tauri/src/modules/connection/mod.rs:453`
  - `src-tauri/src/modules/connection/mod.rs:458`
  - `src-tauri/src/modules/connection/mod.rs:468`
- 结论：
  - `KeyboardInteractiveCoordinator::request()` 先把 `request_id` 写入 `pending`，再 `app.emit(...)`。
  - 如果 `emit` 失败，函数直接报错返回，但 `pending` 中的条目不会回收。
- 影响：
  - 会留下悬空请求，持续污染后续认证流程；长期看属于状态泄露问题。

### 4. 安全面补充判断

- 凭据持久化链路整体比预期安全：
  - 后端保存连接配置时会脱敏保存认证字段，见 `src-tauri/src/modules/connection/mod.rs:1401` 与 `src-tauri/src/modules/connection/mod.rs:651`。
  - `CredentialManager` 在安全存储不可用时拒绝回退到明文保存，见 `src-tauri/src/modules/credential/mod.rs:34`。
- 这意味着当前主要安全问题不在“明文落库”，而在“锁屏绕过”和“临时文件处理”。

## 代码质量

### 1. High: 全局 `RwLock<DefaultConnectionManager>` 持锁期间执行阻塞式网络/SSH 操作

- 证据：
  - `src-tauri/src/lib.rs:103`
  - `src-tauri/src/modules/connection/mod.rs:1045`
  - `src-tauri/src/modules/connection/mod.rs:1831`
- 结论：
  - Tauri command 先拿全局 `RwLock`，随后在 manager 内部执行 `runtime.block_on(...)` 的连接检查、SSH channel 打开、PTY 初始化等操作。
  - 这会把慢连接、慢终端启动、慢网络 I/O 放进粗粒度锁保护区。
- 影响：
  - 其他连接/终端相关命令会被串行阻塞。
  - 并发场景下更容易出现 UI 卡顿、时序异常和后续难以定位的锁竞争问题。

### 2. Medium: 关键跨层数据退化成 `any`，削弱前后端契约

- 证据：
  - `src/lib/store.ts:42`
  - `src/lib/terminalService.ts:469`
  - `src/components/ConnectionModal.svelte:101`
- 结论：
  - `proxy_type`、连接配置、认证路径等关键结构有多处直接用 `any` 透传。
- 影响：
  - 类型错误只能在运行时暴露。
  - 当前仓库是 Svelte + Tauri + Rust 跨层结构，这类类型洞会明显增加维护成本。

### 3. Medium: 成功/错误消息管理分散，`setTimeout(...set(null))` 模式到处重复

- 证据：
  - `src/lib/connectionService.ts:161`
  - `src/lib/terminalService.ts:54`
  - `src/components/ConnectionModal.svelte:290`
- 结论：
  - 提示消息没有 token、版本号或取消机制，旧定时器会清掉新消息。
- 影响：
  - 这既是 UI 状态竞争点，也是明显的可维护性问题；后续任何新操作都要重复踩同一套消息时序问题。

### 4. Low: “导出敏感信息”能力与后端真实行为不一致，测试也在强化这个误导

- 证据：
  - `src/lib/importExportService.ts:92`
  - `src-tauri/src/modules/connection/mod.rs:1401`
  - `src-tauri/src/modules/connection/mod.rs:1443`
  - `src/lib/importExportService.test.ts:92`
- 结论：
  - 前端仍保留“包含敏感信息导出”的分支和确认文案，但后端返回的连接配置已脱敏，keyring 中的密码不会被导出。
  - 现有测试还在 mock “会返回明文秘密”的场景，与真实实现不一致。
- 影响：
  - 这是文案、行为、测试三者脱节的质量问题，容易误导后续开发和用户预期。

## 潜在 Bug

### 1. High: 分屏创建直接复用保存配置建连，绕过密码补全逻辑

- 证据：
  - `src/lib/terminalService.ts:469`
  - `src/lib/terminalService.ts:528`
- 结论：
  - `ensureConnectConfig()` 已实现“密码型连接但未保存密码时弹窗补全”。
  - 但 `createTerminalSession()` 直接调用 `connectWithKnownHostsPrompt(connection)`，没有走密码补全流程。
- 影响：
  - 已连上的密码型 SSH 会话在创建分屏时，如果密码没有持久化保存，二次建连会直接失败。

### 2. Medium: 连接测试与实际连接的主机密钥处理行为不一致

- 证据：
  - `src/components/ConnectionModal.svelte:302`
  - `src/components/ConnectionModal.svelte:323`
  - `src/lib/terminalService.ts:543`
- 结论：
  - “测试连接”只识别 `HOST_KEY_UNKNOWN|...`。
  - 实际终端连接逻辑已经支持 `HOST_KEY_MISMATCH` 与 `HOST_KEY_UNAVAILABLE`。
- 影响：
  - 主机密钥变更或 `known_hosts` 不可用时，测试连接会直接失败，用户无法完成替换/信任流程。

### 3. Medium: 系统监控功能硬编码 Linux 命令

- 证据：
  - `src/components/SystemMonitorModal.svelte:131`
  - `src/components/SystemMonitorModal.svelte:135`
  - `src/components/TerminalManager.svelte:121`
- 结论：
  - 代码直接依赖 `uptime`、`free -m`、`top -bn1`、`cat /proc/net/dev`、`df -h`。
- 影响：
  - 对 macOS、BSD、很多精简 Linux、网络设备等非标准 GNU/Linux 目标会直接失效。

### 4. Medium: 拖拽上传/部分回退路径会把整个文件一次性读入内存

- 证据：
  - `src/components/file-transfer/FileExplorer.svelte:538`
  - `src/components/file-transfer/FileExplorer.svelte:549`
  - `src/components/file-transfer/FileExplorer.svelte:644`
  - `src/lib/localFsService.ts:132`
- 结论：
  - 常规上传入口有分块逻辑，但拖拽上传和异常回退路径仍然会把整文件读成 `Uint8Array` 再整体发送。
- 影响：
  - 大文件下容易打满 renderer 内存或造成明显卡顿。

### 5. Medium: 导出功能会静默丢失已保存凭据

- 证据：
  - `src/lib/importExportService.ts:92`
  - `src-tauri/src/modules/connection/mod.rs:1401`
  - `src-tauri/src/modules/connection/mod.rs:1443`
- 结论：
  - UI 提示“可导出明文密码/口令”，但真实导出内容拿不到 keyring 里的凭据。
- 影响：
  - 用户会以为备份完整，实际恢复后需要重新补密码；这是实打实的功能性缺陷。

## 竞态条件

### 1. High: 自动重连在异步过程中缓存旧索引，可能回写错终端项

- 证据：
  - `src/lib/terminalService.ts:1681`
  - `src/lib/terminalService.ts:1725`
- 结论：
  - `reconnectTerminalInternal()` 先基于快照记录 `index` 和 `terminalEntry`，经过多次 `await` 之后再用旧索引覆盖 `activeTerminals`。
- 影响：
  - 如果这期间用户关闭、切换、重排标签，可能把错误终端替换掉，导致 UI 状态污染。

### 2. Medium: 传输暂停/恢复没有真正取消运行中的任务，可能启动两个 `executeTransfer`

- 证据：
  - `src/lib/transferQueueService.ts:93`
  - `src/lib/transferQueueService.ts:273`
  - `src/lib/transferQueueService.ts:310`
- 结论：
  - 暂停只是改 store 状态，没有中断底层异步流程。
  - 恢复后会重新排队，而旧任务可能尚未自然退出。
- 影响：
  - 同一传输 ID 可能出现并发写入、偏移错乱、进度污染。

### 3. Medium: 连接删除的 optimistic update 在并发删除时会发生回滚竞争

- 证据：
  - `src/lib/connectionService.ts:168`
  - `src/lib/connectionService.ts:184`
- 结论：
  - 本地先删，失败后整表 `loadConnections()` 回滚。
  - 如果两个删除同时进行，先失败的那个会把另一个“仍在后端处理中”的连接重新拉回 UI。
- 影响：
  - 会出现连接短暂“复活”的错觉，导致列表状态不一致。

### 4. Medium: 系统监控轮询没有并发保护，慢请求会覆盖新结果

- 证据：
  - `src/components/SystemMonitorModal.svelte:131`
- 结论：
  - `fetchData()` 内部直接 `Promise.all(...)`，但没有“本轮尚未完成则跳过下轮”的并发门控。
- 影响：
  - 慢链路下老结果可能晚到并覆盖新结果。

### 5. Medium: 当前选中终端的状态轮询没有在返回时校验 session 是否仍然匹配

- 证据：
  - `src/components/TerminalManager.svelte:105`
  - `src/components/TerminalManager.svelte:122`
- 结论：
  - 轮询开始时读取 `sessionId`，异步执行 `exec_command` 返回后直接写全局指标。
- 影响：
  - 快速切标签时，上一会话的数据会短暂写到当前 UI。

### 6. Medium: SFTP 缓存会话在断连并发下仍可能被已拿到句柄的请求继续使用

- 证据：
  - `src-tauri/src/modules/sftp/mod.rs:164`
  - `src-tauri/src/modules/sftp/mod.rs:186`
  - `src-tauri/src/modules/sftp/mod.rs:196`
- 结论：
  - 断连侧只是把缓存项移除；已经拿到 `Arc<Mutex<SftpSession>>` 的请求不会同步失效。
- 影响：
  - 并发断连时，文件操作可能继续打到正在销毁的会话上，表现为随机失败或状态不稳定。

## 总结

本轮并行审查没有发现“凭据明文落库”这类更直接的高危实现，但发现了两个优先级最高的问题：

- 应用锁只是前端遮罩，没有形成后端权限边界。
- 连接/终端核心路径存在多个基于旧快照和粗粒度锁的并发设计问题。

如果要排序处理，建议优先级如下：

1. 给后端高敏感 command 增加统一的 app-lock 校验。
2. 修正终端重连、传输暂停/恢复、删除连接回滚这些明确竞态。
3. 重构全局 `RwLock + block_on` 的持锁模式。
4. 修复 RDP 临时文件策略与导出文案/行为不一致问题。
