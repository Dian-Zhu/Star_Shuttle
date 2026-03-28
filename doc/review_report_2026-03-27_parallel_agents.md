# 工程审查报告（并行 Agent 汇总）

日期：2026-03-27
仓库：`/home/rust/star_-shuttle`

本次审查按你的要求拆成 4 个独立方向并行执行，并由主线程交叉复核：

- 安全漏洞：`Lorentz`
- 代码质量：`Euclid`
- 潜在 Bug：`Ohm`
- 竞态条件：`Peirce`

本次结论基于“当前工作树未提交状态”，不是历史提交。

## 验证结果

- `npm run check`：通过
- `npm test`：通过
- `cargo test`：通过

这些结果说明当前代码没有明显的编译、类型或单测断裂，但不代表逻辑层、授权边界和并发时序没有问题。

## 总结

优先级最高的 4 个问题：

1. `local_fs_stat` 当前可以为任意绝对路径签发访问令牌，实际绕过了 `allowed_roots` 设计。
2. 终端重连后 `sessionId`、`terminalPool` 和 UI 挂载路径没有一起迁移，存在“新会话 ID + 旧 xterm 实例”错位。
3. `scp_upload` 的 SCP 协议头直接拼接文件名，没有过滤换行/控制字符。
4. `sftp_write` / `scp_upload` 命令路径没有和读取路径对齐的 body 大小限制，存在内存/IPC DoS 窗口。

---

## 安全漏洞

### 1. High: `local_fs_stat` 可为任意绝对路径签发访问令牌，导致本地文件访问边界失效

- 证据：
  - `src-tauri/src/modules/local_fs/mod.rs:161`
  - `src-tauri/src/modules/local_fs/mod.rs:289`
  - `src-tauri/src/modules/local_fs/mod.rs:305`
  - `src/lib/localFsService.ts:76`
- 问题：
  - `local_fs_stat(path, access_mode)` 在 `access_mode` 存在时，直接调用 `issue_path_grant()`，没有先执行 `ensure_path_in_allowed_roots()`，也没有任何“该路径必须来自系统文件选择器”的后端约束。
  - 前端只要能调用这个命令，就能先拿 token，再调用 `local_fs_open_read` / `local_fs_open_write` / `local_fs_read_text` / `local_fs_write_text` 访问任意绝对路径。
- 影响：
  - 现在的真实边界不是“只允许 temp 或用户显式授权路径”，而是“前端可自助申请任意绝对路径访问”。
  - 一旦渲染层被注入脚本、依赖被污染，或未来出现误调用，就可以读写用户本地敏感文件。

### 2. Medium: `scp_upload` 对 SCP 协议头中的文件名未做过滤，存在协议头注入风险

- 证据：
  - `src-tauri/src/modules/sftp/mod.rs:795`
  - `src-tauri/src/modules/sftp/mod.rs:804`
- 问题：
  - 上传时目录部分经过了 `shell_quote()`，但真正进入 SCP header 的 `file_name` 直接拼进了 `C0644 <size> <name>\n`。
  - 如果文件名包含换行或其他控制字符，远端 `scp -t` 会按协议解析额外字段或额外命令。
- 影响：
  - 这不是本地 shell 注入，但属于 SCP 协议级注入窗口，可能导致错误文件名、额外协议帧或异常远端行为。

### 3. Medium: 上传命令缺少 body 大小上限，存在本地内存/IPC DoS 窗口

- 证据：
  - `src-tauri/src/modules/sftp/mod.rs:906`
  - `src-tauri/src/modules/sftp/mod.rs:976`
  - `src-tauri/src/modules/sftp/mod.rs:1058`
- 问题：
  - `body_bytes()` 会把整个请求体一次性装入内存。
  - 读取路径已经有 `MAX_SFTP_READ_BYTES` / `MAX_SFTP_CHUNK_BYTES`，但 `sftp_write` 和 `scp_upload` 没有对上传体积做同等级限制。
- 影响：
  - 正常 UI 现在按 chunk 调用，短期不容易触发；但命令本身对未来调用方和异常前端输入不设防，容易造成内存峰值失控。

---

## 代码质量

### 1. High: 终端生命周期抽象已经被 UI 层绕开，`TerminalInstance` / `TerminalPool` 处于半失效状态

- 证据：
  - `src/lib/terminalInstance.ts:113`
  - `src/lib/terminalInstance.ts:161`
  - `src/components/terminal/TerminalPane.svelte:156`
  - `src/components/terminal/TerminalPane.svelte:174`
  - `src/components/terminal/SplitPane.svelte:17`
- 问题：
  - `TerminalInstance` 提供了 `mount()` / `unmount()` 生命周期抽象，但 `TerminalPane` 在已有终端场景下直接搬运 DOM 节点并直接从 `terminalPool` 取实例，没有通过 `TerminalInstance` 统一挂载。
  - 结果是“池中的实例状态”“真实 DOM 所在位置”“Svelte 组件树状态”三者不再天然一致。
- 影响：
  - 后续排查重连、分屏、销毁、事件绑定问题时，很难再相信某个单一抽象层的状态。

### 2. Medium: Host-key 挑战协议在两个前端入口重复实现，后续极易漂移

- 证据：
  - `src/components/ConnectionModal.svelte:305`
  - `src/components/ConnectionModal.svelte:363`
  - `src/lib/terminalService.ts:551`
  - `src/lib/terminalService.ts:589`
- 问题：
  - `ConnectionModal.svelte` 和 `terminalService.ts` 各自维护一套 marker、JSON 解析、确认文案和保存逻辑。
  - 当前两套逻辑大体一致，但已经是独立实现。
- 影响：
  - 一旦后端 payload 字段、错误 marker、UI 文案或确认策略变化，两个入口很容易只修一处，形成隐性分叉。

### 3. Medium: 连接配置归一化采用静默降级，异常数据会被前端“改写后再保存”

- 证据：
  - `src/lib/connectionService.ts:188`
  - `src/lib/connectionService.ts:240`
  - `src/lib/connectionService.ts:322`
  - `src/lib/connectionService.ts:421`
  - `src/lib/connectionService.ts:484`
- 问题：
  - 非法或未知 `auth_method` 会被静默降级成 `KeyboardInteractive`。
  - 非法或未知 `proxy_type` 会被静默降级成 `None`。
  - 这类对象一旦经过前端编辑再保存，原始异常字段会被覆盖掉。
- 影响：
  - 这降低了前后端契约的可观测性，出现兼容性问题时更难定位，也可能让用户无感丢配置。

### 4. Medium: `LocalFsService` 公共 API 只迁移了一半，令牌模型不一致

- 证据：
  - `src/lib/localFsService.ts:76`
  - `src/lib/localFsService.ts:88`
  - `src/lib/localFsService.ts:154`
  - `src/lib/localFsService.ts:185`
- 问题：
  - `requestPathAccess()` 已经成为新模型入口，但 `readFile()` / `writeFile()` 仍保留旧式“直接打开”的公共 API。
  - 这两个 API 自身不会申请 token，也不会强制调用方显式传 token。
- 影响：
  - 现在这套服务从接口层看不出哪些方法可安全复用，哪些方法只是历史遗留。

---

## 潜在 Bug

### 1. High: 终端重连后，`sessionId` 与终端实例池键值不一致，可能导致 UI 绑到新 xterm、监听仍挂在旧 xterm

- 证据：
  - `src/lib/terminalService.ts:1781`
  - `src/lib/terminalService.ts:1828`
  - `src/lib/terminalService.ts:1888`
  - `src/components/TerminalView.svelte:21`
  - `src/components/terminal/SplitPane.svelte:17`
  - `src/lib/terminalService.ts:834`
- 触发条件：
  - 已打开终端发生重连。
- 问题：
  - 重连逻辑把 `activeTerminals` 中的 `sessionId` 替换成新值，但没有同步迁移 `terminalPool` 里旧实例的 key。
  - `TerminalView` 因 `sessionId` 变化重建布局后，`SplitPane` 会按新 `sessionId` 去池里找实例；找不到时又会新建一个 detached terminal。
  - 但重连前拿到的 `term` 仍被继续用于 `setupTerminalListeners()` 和 `onData()`。
- 错误行为：
  - 最终可能出现“界面挂的是新 xterm，输出监听绑在旧 xterm”这种错位，表现为黑屏、输出丢失、幽灵终端或事件泄漏。

### 2. Medium: `TerminalInstance` 的 blur 监听不是幂等注册，重复订阅后无法完整清理

- 证据：
  - `src/lib/terminalInstance.ts:342`
  - `src/lib/terminalInstance.ts:379`
  - `src/lib/terminalInstance.ts:466`
  - `src/lib/terminalInstance.ts:530`
- 触发条件：
  - 同一实例多次 `on('blur', ...)`，再局部 `off()` 或销毁。
- 问题：
  - `_ensureBlurListener()` 每次都会覆盖 `this.blurListener` 并重新 `addEventListener('blur', ...)`，但没有像 focus 监听那样用布尔位保证只注册一次。
  - `_removeBlurListener()` 只能移除“最后一次赋值”的那个监听器。
- 错误行为：
  - 会留下旧 DOM listener，导致 blur 事件重复触发或销毁后仍残留回调。

### 3. Low: `LocalFsService.readFile()` / `writeFile()` 作为公共 API 已处于半失效状态

- 证据：
  - `src/lib/localFsService.ts:154`
  - `src/lib/localFsService.ts:185`
- 触发条件：
  - 未来新调用方直接复用这两个方法处理用户选择的非 temp 路径。
- 问题：
  - 这两个 API 没有衔接新的 token 机制。
- 错误行为：
  - 调用方很容易得到“Access to local path is denied”一类看似随机的运行时失败。

---

## 竞态条件

### 1. High: 重连与 UI 重挂载之间存在时序竞争，会放大终端实例错位

- 证据：
  - `src/lib/terminalService.ts:1820`
  - `src/lib/terminalService.ts:1888`
  - `src/components/TerminalView.svelte:21`
  - `src/components/terminal/SplitPane.svelte:78`
- 并发参与方：
  - `reconnectTerminalInternal()` 的状态替换
  - Svelte 因 `sessionId` 变化触发的重新渲染
  - `SplitPane` 从 `terminalPool` 读取现存实例
- 交错顺序：
  - 先替换 store 中的 `sessionId`
  - 再触发 UI 以新 `sessionId` 重新找实例
  - 但旧实例还没迁移到新 key，监听绑定又继续使用旧 `term`
- 后果：
  - 这不是单纯的“代码质量差”，而是标准的时序竞争，结果取决于渲染与监听重建谁先完成。

### 2. Medium: SCP 传输与断连之间没有 generation 失效校验，断开后远端修改仍可能完成

- 证据：
  - `src-tauri/src/modules/sftp/mod.rs:261`
  - `src-tauri/src/modules/sftp/mod.rs:789`
  - `src-tauri/src/modules/sftp/mod.rs:818`
- 并发参与方：
  - `remove_session()` 对 SFTP session 的移除/代际递增
  - `scp_upload()` / `scp_download()` 的持续 I/O
- 交错顺序：
  - 用户或系统先触发断连
  - 但 SCP 路径已经拿到独立 channel，之后不再检查 generation
- 后果：
  - UI 可能已经认定会话失效，但远端上传/下载仍继续推进，形成“已断开但远端仍变化”的竞态窗口。

---

## 建议处理顺序

1. 先修 `local_fs_stat` 的授权模型。
   目标是让 token 只能来自后端可信授权链路，而不是前端自行申请。
2. 再修终端重连后的实例迁移。
   至少要让 `activeTerminals`、`terminalPool`、`TerminalView` 对同一个会话使用同一份实例身份。
3. 为 `sftp_write` / `scp_upload` 增加统一 body 限制，并补 SCP 文件名过滤。
4. 收敛 host-key 前端协议实现，避免 `ConnectionModal` 和 `terminalService` 双份维护。
5. 清理 `LocalFsService` 的历史 API，明确哪些接口必须显式带 token。

## 备注

- 4 个独立 Agent 已全部完成，最终文档由主线程基于它们的结论做了去重和复核。
- 这份报告只写“当前代码里能成立的问题”；没有把纯风格偏好或证据不足的猜测塞进结果里。
