# 当前工程复审报告

日期：2026-03-26  
范围：当前工作区代码状态（含未提交修改）  
方式：4 个独立 Agent 并行审查，分别覆盖“安全漏洞 / 代码质量 / 潜在 Bug / 竞态条件”，主线程补充本地验证与关键入口抽查。

## 本地验证

- `cargo check`：通过
- `cargo test`：通过，12/12
- `npm run check`：通过，0 error / 0 warning
- `npm test`：通过，4/4
- `npm run build`：通过

## 总体结论

当前工程相比上一轮已经明显收敛，未发现新的明文凭据落库回归，构建与测试也全部通过；但仍存在 4 组需要优先处理的高风险问题：

1. 渲染层权限边界过宽：`fs` capability 覆盖 `$HOME/**` 等大范围路径，CSP 也过于宽松，一旦渲染层被注入脚本，后果会被显著放大。
2. 终端生命周期仍有实质性缺陷：关闭标签页不等于断开后端连接，自动重连与手动关闭之间也存在状态漂移。
3. SFTP session 并发创建与断连交错仍有竞态：会出现同一 `session_id` 下多条底层 channel 或“断开后又被旧请求复活”的情况。
4. 终端相关代码仍过度集中在单一大模块，继续放大回归面和维护成本。

---

## 一、安全漏洞

### 1. 高：渲染层一旦被注入脚本，即可读写大范围本地文件并向外网回传

- 文件：
  - `src-tauri/capabilities/default.json:15`
  - `src-tauri/capabilities/default.json:17`
  - `src-tauri/capabilities/default.json:30`
  - `src-tauri/capabilities/default.json:39`
  - `src-tauri/capabilities/default.json:48`
  - `src-tauri/capabilities/default.json:57`
  - `src-tauri/capabilities/default.json:66`
  - `src-tauri/capabilities/default.json:75`
  - `src-tauri/tauri.conf.json:25`
- 现状：
  - `fs` capability 允许对 `$HOME/**`、`$DESKTOP/**`、`$DOCUMENT/**`、`$DOWNLOAD/**` 进行读写、创建、删除、重命名。
  - CSP 允许 `unsafe-inline`、`unsafe-eval`、`http:`、`https:`、`ws:`、`wss:`。
- 风险：
  - 一旦渲染层获得脚本执行能力，攻击面会直接扩大到用户本地文件系统和外部网络。
- 建议：
  - 收缩 `fs` capability 到最小必需范围。
  - 优先依赖用户显式选择的文件路径，而不是全局目录权限。
  - 进一步收紧 CSP，尤其移除 `unsafe-eval`，尽量收缩 `unsafe-inline` 和广域 `connect-src`。

### 2. 中：应用锁仍是 UI 语义，不是后端强制安全边界

- 文件：
  - `src/components/Layout.svelte:175`
  - `src/components/AppLockOverlay.svelte:17`
  - `src-tauri/src/lib.rs:547`
  - `src-tauri/src/lib.rs:549`
  - `src-tauri/src/lib.rs:554`
  - `src-tauri/src/lib.rs:562`
  - `src-tauri/src/lib.rs:574`
- 现状：
  - 前端通过 `isLocked` 控制遮罩显示；后端高权限命令面没有统一的“锁定态拒绝执行”。
- 风险：
  - 当前更像“界面锁屏”，而不是“命令面锁定”。
  - 若渲染层仍能执行 JS，锁定期间仍可能调用高权限命令。
- 建议：
  - 在 Rust 后端维护真实锁定态，并在高权限命令入口统一拦截。

### 3. 低：RDP 启动会落地可预测的临时文件，且缺少清理

- 文件：
  - `src-tauri/src/lib.rs:109`
  - `src-tauri/src/lib.rs:113`
  - `src-tauri/src/lib.rs:120`
  - `src-tauri/src/lib.rs:137`
- 现状：
  - `launch_rdp` 会在系统临时目录创建 `starshuttle-{host}-{port}.rdp`。
- 风险：
  - 泄露目标主机、端口、用户名等连接元数据。
- 建议：
  - 改为随机文件名、最小权限，并在启动后尽快清理。

---

## 二、代码质量

### 1. 高：`terminalService.ts` 已成为终端相关的 God module

- 文件：`src/lib/terminalService.ts:1`
- 问题：
  - 同时承担连接建立、终端初始化、事件监听、重连、状态恢复、UI 消息和 store 写入。
- 影响：
  - 修改一个终端行为，回归面会扩散到多个职责边界。
- 建议：
  - 至少拆分为 `session lifecycle`、`terminal runtime`、`reconnect/status listeners` 三层。

### 2. 中高：`createBackendConfig()` 混入隐藏副作用

- 文件：`src/lib/connectionService.ts:333`
- 问题：
  - DTO 构造/校验函数内部直接读写 `connectionGroups`，并自动创建分组。
- 影响：
  - 保存失败时，前端仍可能留下未被实际使用的脏分组。
- 建议：
  - 把“构造配置”和“更新 UI 分组”拆开，前者保持纯函数。

### 3. 中：本地文件服务核心 I/O 仍大量使用 `any`

- 文件：`src/lib/localFsService.ts:13`
- 问题：
  - `openFile/openWriteFile/readChunk/writeChunk/closeFile` 等关键链路缺少真实类型边界。
- 影响：
  - 文件传输接口变更更容易延后到运行期爆炸。
- 建议：
  - 为 Tauri FS handle 和目录项补最小可用类型。

### 4. 中：敏感字段脱敏逻辑仍有重复实现

- 文件：
  - `src/lib/importExportService.ts:10`
  - `src/components/ConnectionModal.svelte:187`
- 问题：
  - 导出脱敏和连接配置脱敏分别维护，后续字段演进时容易漂移。
- 影响：
  - 一处修了，一处漏了，会重新引入敏感字段泄露风险。
- 建议：
  - 抽成单一共享 sanitize/serialize 层。

### 5. 中：`closeBackendTerminalsBestEffort()` 的实现和语义不一致

- 文件：`src/lib/terminalService.ts:1289`
- 问题：
  - 写成了 10 次循环，但首次失败就直接 `return`，实际上没有重试。
- 影响：
  - 代码阅读者会误以为这里具备关闭重试保障。
- 建议：
  - 要么实现真实重试和退避，要么删掉循环保留单次调用。

---

## 三、潜在 Bug

### 1. 高：关闭标签页/面板只关闭终端，不断开后端会话

- 文件：
  - `src/lib/terminalService.ts:1180`
  - `src/components/TerminalView.svelte:64`
  - `src/components/TerminalView.svelte:198`
  - `src/components/TitleBar.svelte:92`
  - `src/components/Layout.svelte:351`
- 复现：
  - 建立会话后，通过标签关闭、快捷键或分屏关闭入口关闭终端。
- 风险：
  - 前端标签消失，但后端 session 仍保持连接，形成“幽灵连接”。
- 建议：
  - 所有用户可见关闭入口统一走 `disconnectTerminal`，或明确区分“只销毁 UI”和“关闭并断开连接”。

### 2. 中：导入连接会按原始 ID 覆盖已有配置

- 文件：`src/lib/importExportService.ts:160`
- 复现：
  - 重复导入同一备份，或导入与本地存在同 ID 的配置。
- 风险：
  - 用户预期是“追加”，实际却是静默覆盖。
- 建议：
  - 默认为导入项生成新 ID，若支持覆盖则显式提示。

### 3. 中：文件浏览器“返回上一级”会把 `..` 写入当前路径状态

- 文件：
  - `src/components/file-transfer/FileExplorer.svelte:218`
  - `src/components/file-transfer/FileExplorer.svelte:260`
  - `src/components/file-transfer/FileExplorer.svelte:791`
  - `src/lib/sftpService.ts:20`
- 复现：
  - 从绝对路径目录返回上一级后，继续执行重命名、删除、下载、复制粘贴等操作。
- 风险：
  - UI 看起来在父目录，后续操作却基于相对路径 `..`，可能打到错误目标。
- 建议：
  - 返回上一级时先解析真实父目录，避免把字面量 `..` 存入 `currentPath`。

### 4. 中：终端初始化失败时会留下已注册实例和监听器

- 文件：
  - `src/lib/terminalService.ts:672`
  - `src/lib/terminalService.ts:741`
  - `src/lib/terminalService.ts:803`
  - `src/lib/terminalService.ts:895`
  - `src/lib/terminalService.ts:933`
- 复现：
  - 连接成功后，在 `start_terminal` 前后快速断开，或后端 `start_terminal` 失败。
- 风险：
  - 可能残留 `terminalPool` 注册、输入输出监听器与缓冲状态。
- 建议：
  - 把注册动作移到成功之后，或在失败分支统一执行清理。

### 5. 中：自动重连已排队时手动关闭标签，不会清理重连定时器

- 文件：
  - `src/lib/terminalService.ts:1180`
  - `src/lib/terminalService.ts:1255`
  - `src/lib/terminalService.ts:1602`
  - `src/lib/terminalService.ts:1656`
- 复现：
  - 自动重连倒计时期间手动关闭标签。
- 风险：
  - 后台仍可能继续重连一个已经被关闭的 session。
- 建议：
  - 在所有关闭路径中统一清理 `reconnectTimers/reconnectKeyListeners/reconnectAttempts`。

---

## 四、竞态条件

### 1. 高：SFTP `get_session()` 存在并发双创建

- 文件：`src-tauri/src/modules/sftp/mod.rs:164`
- 并发场景：
  - 两个请求同时发现缓存里没有该 `session_id`，随后各自并发创建新的 `SftpSession`。
- 后果：
  - 同一逻辑会话下短时间出现多条底层 SFTP channel，缓存状态与真实连接不一致。
- 建议：
  - 引入 per-session 单飞控制或 `Pending` 占位态。

### 2. 高：`disconnect()` 与 `get_session()` 交错时，旧请求可把断开的 session 重新写回缓存

- 文件：
  - `src-tauri/src/modules/sftp/mod.rs:175`
  - `src-tauri/src/lib.rs:188`
  - `src-tauri/src/modules/connection/mod.rs:1310`
- 并发场景：
  - 线程 A 已拿到 `ssh_conn`，线程 B 同时执行断连并清缓存；A 之后仍可能成功创建 channel 并重新插入 SFTP cache。
- 后果：
  - 逻辑上已经断开的会话，仍可能被旧请求“复活”。
- 建议：
  - 在创建完成后、插入缓存前再次校验 session 仍为 `Connected`，更稳妥的方式是引入关闭 epoch/屏障。

### 3. 高：`reconnectTerminal()` 没有 in-flight 防重入保护

- 文件：`src/lib/terminalService.ts:1656`
- 并发场景：
  - 自动重连、按键重连、外部重复调用可能同时进入重连流程。
- 后果：
  - 后端可能建立多个新会话，但前端只跟踪最后一次写入的 `sessionId`。
- 建议：
  - 增加 `reconnectInFlight` 映射，只允许首个调用执行，后续调用复用同一个 promise 或直接返回。

### 4. 中，脆弱点：`TerminalPane` 异步初始化缺少取消标记

- 文件：`src/components/terminal/TerminalPane.svelte:131`
- 并发场景：
  - pane 在异步初始化过程中被关闭、重排或切换 session。
- 后果：
  - 晚到的初始化可能继续给旧 DOM 或旧 terminal 绑定事件，造成重复 active、错绑监听器或晚到挂载。
- 建议：
  - 增加 `destroyed/cancelled` 标记，并在 `onDestroy` 中显式移除命名事件处理器。

---

## 优先级建议

建议按下面顺序处理：

1. 先修 SFTP 并发创建与断连复活问题。
2. 统一终端关闭语义，消除“关闭 UI 但不断开后端”的幽灵连接。
3. 给 `reconnectTerminal()` 增加单飞保护，并统一清理重连状态。
4. 收缩 `fs` capability 与 CSP。
5. 处理文件浏览器父目录路径状态和导入覆盖语义。
6. 最后做 `terminalService.ts` 拆分与类型边界治理。
