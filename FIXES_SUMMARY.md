# xterm 6.0 修复总结

## 修复的核心问题

### 1. xterm 6.0 API 兼容性 ✅
**问题**: `term.write()` 方法在 6.0 中改为返回 Promise，不再接受回调
**修复**:
- 重构 `flushOutput()` 函数使用 `await term.write(payload)`
- 更新所有 `term.write()` 调用为 Promise 模式或 fire-and-forget 模式

**影响文件**:
- `src/lib/terminalService.ts` - 核心输出处理逻辑

### 2. 输出状态初始化顺序 ✅
**问题**: `applyPausedState()` 在订阅设置之前调用，导致初始状态不正确
**修复**:
- 先设置 `selectedTerminalIndex` 和 `activeTerminals` 订阅
- 然后再调用 `applyPausedState()` 确保初始状态正确

**影响文件**:
- `src/lib/terminalService.ts` - `setupTerminalListeners` 函数

### 3. 容器可见性处理 ✅
**问题**: 终端可能在容器 `display: none` 时初始化
**修复**:
- 移除 `isVisible` 检查，终端在 `onMount` 时立即初始化
- 改进 `isVisible` 响应式语句，移除不必要的延迟
- 添加容器状态日志

**影响文件**:
- `src/components/TerminalView.svelte`

### 4. CSS 修复 ✅
**问题**: xterm 6.0 可能需要额外的 CSS 样式
**修复**:
- 添加 `.xterm-viewport`, `.xterm-screen`, `.xterm-rows` 样式
- 确保容器有正确的尺寸和溢出处理

**影响文件**:
- `src/app.css`

## 性能优化

### 输出处理
- **自适应批处理**: 根据性能动态调整块大小（32KB - 2MB）
- **智能缓冲区修剪**: 在 4096 块或 70% 时修剪
- **慢写入检测**: 跟踪连续慢写入并降低预算
- **最小块大小**: 确保批处理效率（1024 字节）

### 输入处理
- **动态阈值**: 根据待处理块数调整（1024-2048 字节）
- **更好的状态跟踪**: 记录刷新时间和待处理块数

### 终端初始化
- **新增选项**: `altClickMovesCursor`, `scrollSensitivity`, `fastScrollSensitivity`, `rightClickSelectsWord`
- **增强的 WebGL 加载**: 更好的错误处理和回退
- **安全 URI 验证**: WebLinks addon 的安全增强

## 调试工具

### 统一日志系统
- `log.info()` - 信息日志
- `log.warn()` - 警告日志
- `log.error()` - 错误日志（带上下文）
- `log.perf()` - 性能监控（慢操作 >10ms）

### 关键日志点
- 终端初始化：容器尺寸、终端尺寸、渲染器类型
- 输出事件：数据接收、暂停状态、块添加
- 写入操作：负载大小、写入时间、性能指标
- 可见性变化：容器尺寸、终端尺寸

## 测试步骤

### 1. 基本功能测试
- [ ] 打开应用
- [ ] 创建新的 SSH 连接
- [ ] 检查控制台日志
- [ ] 验证终端显示初始化消息

### 2. 可见性测试
- [ ] 连接到多个终端
- [ ] 切换标签页
- [ ] 检查 `isVisible` 变化日志
- [ ] 验证终端正确显示/隐藏

### 3. 性能测试
- [ ] 执行长时间运行的命令
- [ ] 检查 `perf` 日志
- [ ] 验证批处理工作正常
- [ ] 检查内存使用

### 4. 错误恢复测试
- [ ] 断开连接
- [ ] 触发错误场景
- [ ] 检查错误日志
- [ ] 验证恢复机制

## 预期日志输出

### 成功的初始化
```
[TermInit] Terminal opened in container { sessionId: "xxx", containerSize: {...}, terminalSize: {...} }
[TermInit] WebGL renderer loaded successfully
[TermInit] Layout stabilized after 1 attempts
[TermOutput] Setting up terminal listeners { sessionId: "xxx" }
[TermOutput] Test message written to terminal { sessionId: "xxx" }
[TermInit] Terminal session started successfully { sessionId: "xxx", cols: 80, rows: 24 }
```

### 成功的输出流
```
[TermOutput] Received output event { sessionId: "xxx", hasPayload: true, hasData: true, dataLength: 1234, isPaused: false }
[TermOutput] Chunk added { sessionId: "xxx", chunkIndex: 0, isWriting: false }
[TermOutput] Starting flush { sessionId: "xxx", totalChunks: 1, chunkBudget: 262144 }
[TermOutput] Writing to terminal { sessionId: "xxx", payloadLength: 1234, payloadPreview: "..." }
```

### 可见性变化
```
[TerminalView] Terminal became visible, fitting...
[TerminalView] Terminal dimensions: { cols: 80, rows: 24 }
```

## 故障排除

### 问题: 终端仍然空白

#### 检查 1: 验证初始化
打开控制台，查找：
```
[TermInit] Terminal opened in container
```
- ✅ 看到 `containerSize.width > 0` 且 `containerSize.height > 0`
- ❌ 如果尺寸为 0，容器有问题

#### 检查 2: 验证测试消息
查找：
```
[TermOutput] Test message written to terminal
```
- ✅ 如果看到，终端写入正常
- ❌ 如果看不到，终端初始化有问题

#### 检查 3: 验证输出事件
查找：
```
[TermOutput] Received output event
```
- ✅ 如果看到，后端在发送数据
- ❌ 如果看不到，后端问题

#### 检查 4: 验证暂停状态
查找：
```
[TermOutput] Compute paused state { ... isPaused: false }
```
- ✅ `isPaused: false`
- ❌ `isPaused: true` - 输出被暂停

#### 检查 5: 验证写入操作
查找：
```
[TermOutput] Writing to terminal { payloadLength: > 0 }
```
- ✅ `payloadLength` 应该大于 0
- ❌ `payloadLength: 0` - 数据被跳过

### 问题: 终端尺寸错误

**症状**: 终端显示不正确或滚动

**检查**:
```javascript
// 在控制台运行
document.querySelector('.xterm')?.getBoundingClientRect()
document.querySelector('.xterm-rows')?.getBoundingClientRect()
```

**修复**: 手动触发 resize
```javascript
window.dispatchEvent(new Event('resize'))
```

### 问题: 性能问题

**症状**: 终端响应慢或卡顿

**检查**: 查找 `perf` 日志
```
[PERF] [TermOutput] write took 45.2ms
```

**解决方案**:
- 减少终端会话数
- 检查 WebGL 渲染器是否工作
- 调整 `chunkBudget` 参数

## 下一步

如果问题仍然存在：

1. **收集完整的控制台日志**
   - 打开开发者工具 (F12)
   - 切换到 Console 标签
   - 复制所有日志（包括警告和错误）

2. **检查网络请求**
   - 切换到 Network 标签
   - 过滤 `terminal-output` 事件
   - 验证事件到达

3. **检查后端日志**
   - SSH 连接状态
   - 终端输出是否发送
   - 错误信息

4. **提交 Issue**
   - 包含完整的控制台日志
   - 截图终端空白状态
   - 说明尝试的连接类型（SSH/Telnet）
   - 操作系统和浏览器版本

## 相关文件

- `src/lib/terminalService.ts` - 终端服务核心逻辑
- `src/components/TerminalView.svelte` - 终端视图组件
- `src/app.css` - 全局样式（包含 xterm 样式）
- `DEBUGGING.md` - 详细调试指南
- `package.json` - 依赖版本（xterm 6.0.0）
