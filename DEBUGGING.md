# 终端渲染空白问题调试指南

## 已修复的问题

### 1. xterm 6.0 API 兼容性 ✅
- 将 `term.write()` 从回调 API 改为 Promise API
- 更新所有写入调用以使用新的异步模式

### 2. 输出状态管理 ✅
- 修复了 `setupTerminalListeners` 中的初始状态计算
- 确保订阅在 `applyPausedState()` 调用之前设置

### 3. 容器可见性处理 ✅
- 终端现在在不可见时也会初始化（通过 `initTerminal`）
- 改进了 `isVisible` 变化时的响应逻辑

## 调试日志

### 打开开发者控制台
1. 按 F12 或右键 -> 检查
2. 切换到 Console 标签页

### 关键日志标记

#### 终端初始化
```
[TermInit] Terminal opened in container
[TermInit] WebGL renderer loaded successfully / unavailable
[TermInit] Layout stabilized after X attempts
[TermInit] Terminal session started successfully
```

#### 输出事件
```
[TermOutput] Received output event
[TermOutput] Compute paused state
[TermOutput] Chunk added
[TermOutput] Starting flush
[TermOutput] Writing to terminal
```

#### 可见性变化
```
[TerminalView] Terminal became visible, fitting...
[TerminalView] Terminal dimensions: { cols, rows }
```

## 诊断步骤

### 步骤 1: 检查初始化日志
```
[TermInit] Terminal opened in container
```
- ✅ 看到 `containerSize` 非零值
- ✅ 看到 `terminalSize` 合理（如 80x24）

### 步骤 2: 检查会话启动
```
[TermInit] Terminal session started successfully
```
- ✅ 应该看到此日志
- ❌ 如果看到 "Failed to start terminal session"，说明后端连接失败

### 步骤 3: 检查输出事件
```
[TermOutput] Received output event
```
- ✅ 应该看到此日志
- ❌ 如果没有看到，说明后端没有发送数据

### 步骤 4: 检查暂停状态
```
[TermOutput] Compute paused state
```
- 查看 `isPaused: false`
- ❌ 如果 `isPaused: true`，终端输出被暂停

### 步骤 5: 检查写入操作
```
[TermOutput] Writing to terminal
```
- 查看 `payloadLength` > 0
- ✅ 看到 `payloadPreview` 有实际内容

## 常见问题

### 问题: "Compute paused state: isPaused: true"
**原因**: 选中的会话 ID 与当前会话 ID 不匹配
**解决**: 点击终端标签页切换到正确的会话

### 问题: "No output events received"
**原因**: 后端没有发送终端输出
**解决**: 检查 SSH 连接状态，尝试重新连接

### 问题: "Writing to terminal: payloadLength: 0"
**原因**: 所有数据块被跳过或为空
**解决**: 检查 `outputState.paused` 和 `outputState.chunkIndex`

### 问题: "containerSize: { width: 0, height: 0 }"
**原因**: 终端容器不可见或尺寸为零
**解决**: 确保终端标签页可见

## 手动测试

### 在控制台手动测试
```javascript
// 1. 检查终端实例
console.log('Terminals:', import('./lib/store.js').then(m => get(m.activeTerminals)));

// 2. 手动写入测试
import('./lib/terminalService.js').then(m => {
  // 获取终端实例并手动写入
  console.log('Terminal service loaded');
});
```

### 强制刷新
```javascript
// 重新 fit 终端
document.querySelectorAll('.xterm').forEach(el => {
  el.dispatchEvent(new Event('resize'));
});
```

## 网络检查

打开 Network 标签，检查：
- ✅ `start_terminal` 调用成功（状态 200）
- ✅ `terminal-output-{sessionId}` 事件到达
- ❌ 检查是否有失败的请求或错误

## 后端验证

如果问题仍然存在，检查后端：
1. SSH 连接是否成功建立
2. 终端输出是否被发送
3. `start_terminal` 命令是否正确执行

## 下一步

如果以上都无法解决问题：
1. 收集完整的控制台日志
2. 截图终端空白的状态
3. 检查 Network 标签的事件
4. 提交 Issue 包含以上信息
