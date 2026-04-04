# Star Shuttle — AI 功能开发计划

## 概述

在现有 SSH 远程管理应用中引入 AI 能力，分为 **Chat 模式** 和 **Agent 模式** 两大功能模块。

- **Chat 模式**：用户与 AI 对话，AI 可读取当前终端内容，辅助分析问题（如错误诊断、命令建议、日志分析）
- **Agent 模式**：AI 直接操控终端，执行命令、查询内容，并通过 **权限沙箱** 拦截高危命令进行二次确认

---

## 技术架构

```
┌─────────────────────────────────────────────────┐
│                   前端 (Svelte 5)                │
│  ┌───────────┐  ┌────────────┐  ┌─────────────┐ │
│  │ Chat 面板  │  │ Agent 面板  │  │ 确认弹窗    │ │
│  └─────┬─────┘  └──────┬─────┘  └──────┬──────┘ │
│        │               │               │        │
│  ┌─────┴───────────────┴───────────────┴──────┐ │
│  │           AI Service (前端服务层)            │ │
│  │  - 消息管理 / 上下文构建 / 流式响应处理      │ │
│  └──────────────────┬──────────────────────────┘ │
└─────────────────────┼───────────────────────────┘
                      │ Tauri IPC (invoke / events)
┌─────────────────────┼───────────────────────────┐
│              后端 (Rust / Tauri)                  │
│  ┌──────────────────┴──────────────────────────┐ │
│  │             AI Module (新增)                 │ │
│  │  ┌─────────────┐  ┌──────────────────────┐  │ │
│  │  │ LLM Client  │  │ Terminal Context      │  │ │
│  │  │ (API 调用)   │  │ Collector (终端读取)  │  │ │
│  │  └─────────────┘  └──────────────────────┘  │ │
│  │  ┌─────────────┐  ┌──────────────────────┐  │ │
│  │  │ Agent       │  │ Permission Sandbox    │  │ │
│  │  │ Executor    │  │ (权限沙箱)            │  │ │
│  │  └─────────────┘  └──────────────────────┘  │ │
│  └─────────────────────────────────────────────┘ │
│                                                   │
│  ┌───────────────────────────────────────┐       │
│  │  现有模块 (connection / terminal /     │       │
│  │  sftp / session / db)                 │       │
│  └───────────────────────────────────────┘       │
└───────────────────────────────────────────────────┘
```

---

## 阶段一：基础设施搭建（~1.5 周）

### 1.1 AI 后端模块骨架

**目标**：建立 AI 模块的 Rust 后端基础结构

- [ ] 创建 `src-tauri/src/modules/ai/` 模块目录
  - `mod.rs` — 模块入口
  - `config.rs` — AI 配置（API Key、模型选择、温度参数等）
  - `client.rs` — LLM API 客户端（支持 OpenAI 兼容 API）
  - `types.rs` — 消息、会话、工具调用等类型定义
- [ ] 在 `Cargo.toml` 添加依赖：`reqwest`（HTTP）、`tokio-stream`（SSE 流式）、`serde` 序列化
- [ ] 实现基础 LLM 客户端
  - 支持 OpenAI API 格式（兼容 Claude / DeepSeek / 本地 Ollama 等）
  - 支持流式响应（SSE）
  - 错误处理与重试机制
- [ ] 在 settings 中新增 AI 配置页面入口

### 1.2 AI 配置与存储

**目标**：用户可配置 AI 服务商和参数

- [ ] 数据库新增 `ai_config` 表（provider、api_key、model、base_url、temperature 等）
- [ ] 前端 Settings 中新增 "AI 设置" Tab
  - API Provider 选择（OpenAI / Claude / DeepSeek / 自定义）
  - API Key 输入（加密存储）
  - Model 选择
  - Base URL（支持自定义端点 / 本地 Ollama）
- [ ] API Key 安全存储（使用现有 `keyring` crate 或 AES 加密到 SQLite）

### 1.3 终端上下文采集器

**目标**：能够捕获当前终端的可见内容，供 AI 读取

- [ ] 在 `terminal` 模块中新增 `context_collector.rs`
  - 获取当前活跃终端的屏幕缓冲区内容（最近 N 行）
  - 获取终端 scrollback 历史（可配置行数）
  - 支持获取指定 session 的终端内容
- [ ] 新增 Tauri Command：`get_terminal_context`
  - 参数：`session_id`、`lines`（行数）、`include_scrollback`
  - 返回：终端文本内容 + 元信息（连接信息、当前目录等）
- [ ] 前端 `terminalService` 中封装 `getTerminalContext()` 方法

---

## 阶段二：Chat 模式（~2 周）

### 2.1 Chat 后端核心

**目标**：实现 AI Chat 的后端逻辑

- [ ] `src-tauri/src/modules/ai/chat.rs`
  - 会话管理：创建 / 续接 / 清除对话
  - 消息历史管理（内存 + SQLite 持久化）
  - 上下文注入：自动附加终端内容到系统提示
- [ ] 数据库新增表
  - `ai_conversations`（id, session_id, title, created_at, updated_at）
  - `ai_messages`（id, conversation_id, role, content, context_snapshot, created_at）
- [ ] 新增 Tauri Commands
  - `ai_chat_send` — 发送消息（支持流式事件推送）
  - `ai_chat_history` — 获取对话历史
  - `ai_chat_new` — 新建对话
  - `ai_chat_clear` — 清除对话
- [ ] 流式响应通过 Tauri Event 推送到前端（`ai-chat-stream-{conversationId}`）

### 2.2 Chat 前端 UI

**目标**：实现 Chat 对话界面

- [ ] 新增 `src/components/ai/` 目录
  - `AiChatPanel.svelte` — Chat 主面板（可嵌入右侧栏或独立面板）
  - `ChatMessage.svelte` — 消息气泡组件（支持 Markdown 渲染）
  - `ChatInput.svelte` — 输入框（支持多行、快捷键发送）
  - `ChatHistory.svelte` — 对话历史列表
  - `ContextIndicator.svelte` — 显示当前附加的终端上下文摘要
- [ ] 新增 `src/lib/aiChatService.ts` — 前端 AI Chat 服务
  - 调用 Tauri commands
  - 监听流式事件，实时更新消息内容
  - 管理对话状态
- [ ] Chat 面板集成到主布局
  - 右侧栏 Tab 或底部面板（可切换）
  - 支持拖拽调整大小
  - 一键"附加当前终端内容"按钮

### 2.3 上下文增强

**目标**：AI 能理解终端环境，提供精准辅助

- [ ] 系统提示词模板设计
  - 包含：连接信息（host、OS、shell 类型）、当前目录、最近命令输出
  - 可配置的角色预设（运维专家 / 开发助手 / 安全审计员 等）
- [ ] 上下文自动刷新
  - 每次发送消息时可选择自动附加最新终端内容
  - "引用选中内容" — 终端中选中文本直接发给 AI
- [ ] Markdown 渲染
  - 代码块语法高亮
  - 一键"复制命令"按钮
  - "在终端执行"快捷按钮（仅 Chat 模式下需要用户手动确认）

---

## 阶段三：Agent 模式（~2.5 周）

### 3.1 权限沙箱系统

**目标**：建立命令分级权限控制体系

- [ ] `src-tauri/src/modules/ai/sandbox.rs` — 权限沙箱核心

  **沙箱分两级**：

  #### 标准沙箱（Standard Sandbox）
  - 白名单机制：预定义安全命令列表
    - 只读命令：`ls`, `cat`, `head`, `tail`, `grep`, `find`, `pwd`, `whoami`, `df`, `du`, `ps`, `top`, `free`, `uname`, `date`, `uptime`, `netstat`, `ss`, `ip`, `ifconfig`, `dig`, `nslookup`, `ping`, `traceroute`, `curl`（GET only）, `wget`（dry-run）, `file`, `stat`, `wc`, `sort`, `uniq`, `awk`（只读）, `jq`, `docker ps`, `docker logs`, `docker inspect`, `kubectl get`, `kubectl describe`, `kubectl logs`, `systemctl status`, `journalctl`
  - 自动放行白名单内的命令
  - 白名单外的命令 → 全部拦截，请求用户确认

  #### 严格沙箱（Strict Sandbox）
  - 黑名单机制：显式标记高危命令
    - 高危命令（永久拦截需确认）：`rm -rf /`, `mkfs`, `dd`, `:(){ :|:& };:`, `> /dev/sda`, `chmod -R 777 /`, `shutdown`, `reboot`, `halt`, `init 0`, `kill -9 1`, `iptables -F`, `DROP TABLE`, `DELETE FROM`（无 WHERE）, `userdel`, `passwd root`
    - 敏感命令（拦截需确认）：`rm`（带 `-rf` / `-f`）, `chmod`, `chown`, `systemctl stop/restart`, `docker rm/stop/kill`, `kubectl delete`, `apt remove/purge`, `yum remove`, `pip uninstall`, `crontab -r`, `kill`, `pkill`, `mount/umount`, `fdisk`, `parted`, `iptables`, `firewall-cmd`, `useradd/userdel/usermod`, SQL `DROP/ALTER/TRUNCATE/UPDATE`（无 WHERE）
  - 不在黑名单中的命令 → 自动放行
  - 用户可自定义追加黑名单/白名单规则

- [ ] 命令解析器（`command_parser.rs`）
  - 解析 shell 命令字符串（处理管道、重定向、`&&`、`||`、`;` 等）
  - 提取命令名 + 参数
  - 识别管道链中的危险操作
  - 处理别名和变量展开（最佳努力）

- [ ] 权限判定引擎
  - 输入：命令字符串 + 当前沙箱级别
  - 输出：`Allow` / `Deny(reason)` / `NeedConfirm(reason, risk_level)`
  - 支持正则匹配和模式匹配

- [ ] 数据库新增 `sandbox_rules` 表（用户自定义规则持久化）
- [ ] 数据库新增 `ai_command_audit` 表（所有 Agent 执行的命令审计日志）

### 3.2 Agent 执行引擎

**目标**：AI 能自主规划并执行终端操作

- [ ] `src-tauri/src/modules/ai/agent.rs` — Agent 核心逻辑
  - Tool-use / Function-calling 协议实现
  - 定义可用工具集：
    - `execute_command` — 在指定会话终端执行命令
    - `read_terminal` — 读取终端当前内容
    - `read_file` — 读取远程文件内容（通过 SFTP）
    - `list_directory` — 列出远程目录（通过 SFTP）
    - `get_system_info` — 获取系统信息（OS、内存、磁盘等）
  - 多步骤执行循环：
    1. AI 分析任务 → 生成 tool_call
    2. 命令经过沙箱权限检查
    3. 通过 → 执行并返回结果给 AI
    4. 需确认 → 暂停等待用户确认
    5. AI 根据结果决定下一步或结束

- [ ] 执行状态管理
  - Agent 任务生命周期：`pending` → `running` → `waiting_confirm` → `running` → `completed/failed`
  - 支持中途取消（用户可随时停止 Agent）
  - 超时保护（单条命令超时 / 总任务超时）

- [ ] 新增 Tauri Commands
  - `ai_agent_start` — 启动 Agent 任务（传入自然语言指令）
  - `ai_agent_confirm` — 用户确认/拒绝待确认的命令
  - `ai_agent_cancel` — 取消当前 Agent 任务
  - `ai_agent_status` — 查询 Agent 当前状态

### 3.3 Agent 前端 UI

**目标**：实现 Agent 操控界面和确认交互

- [ ] 新增前端组件
  - `AiAgentPanel.svelte` — Agent 主面板
    - 输入任务指令
    - 实时显示 Agent 思考过程和操作步骤
    - 执行日志时间线（每一步的命令和输出）
  - `CommandConfirmModal.svelte` — 高危命令确认弹窗
    - 显示待执行的命令
    - 风险等级标识（🟡 敏感 / 🔴 高危）
    - 原因说明
    - 确认 / 拒绝 / 全部拒绝 按钮
  - `AgentToolCallStep.svelte` — 单步操作展示组件
    - 展示工具调用名称、参数
    - 命令执行状态（等待 / 执行中 / 成功 / 失败 / 需确认）
    - 可折叠的输出内容
  - `SandboxIndicator.svelte` — 当前沙箱级别指示器

- [ ] 新增 `src/lib/aiAgentService.ts` — 前端 Agent 服务
  - 任务管理
  - 监听 Agent 事件流
  - 确认/拒绝交互

- [ ] Agent 面板集成
  - 与 Chat 面板共享入口，通过 Tab 切换
  - 沙箱模式切换器（标准 / 严格）
  - Agent 执行历史

---

## 阶段四：打磨与增强（~1 周）

### 4.1 用户体验优化

- [ ] Chat/Agent 模式无缝切换
  - Chat 中提到 "帮我执行..." 时，提示切换到 Agent 模式
  - Agent 执行结果可在 Chat 中继续讨论
- [ ] 快捷键
  - `Ctrl+Shift+A` / `Cmd+Shift+A` — 打开 AI 面板
  - `Ctrl+Shift+C` — 将终端选中内容发送给 AI
- [ ] 终端集成
  - 终端右键菜单添加 "Ask AI" 选项
  - 终端中输出错误时，可自动提示 "使用 AI 分析"

### 4.2 安全加固

- [ ] 所有 Agent 命令执行审计日志（who, when, what, result）
- [ ] API Key 加密存储安全审计
- [ ] 沙箱规则不可被 AI 自身修改
- [ ] 命令注入防护（防止 AI 通过拼接绕过沙箱）
  - 检测 `$()`, `` ` ` ``, `\n` 注入
  - 参数转义处理
- [ ] Agent 模式下的命令执行频率限制（防止循环执行）

### 4.3 测试

- [ ] Rust 单元测试
  - 命令解析器测试（各种 shell 语法组合）
  - 沙箱权限判定测试（覆盖白名单、黑名单、边界情况）
  - LLM Client Mock 测试
- [ ] 前端测试
  - Chat 消息渲染测试
  - Agent 状态流转测试
  - 确认弹窗交互测试
- [ ] 集成测试
  - Chat 完整对话流程
  - Agent 执行 + 沙箱拦截 + 确认 完整流程

---

## 开发排期总览

| 阶段 | 内容 | 预估时间 | 优先级 |
|------|------|----------|--------|
| 一 | 基础设施（AI 模块骨架 + 配置 + 终端上下文） | 1.5 周 | P0 |
| 二 | Chat 模式（后端 + 前端 + 上下文增强） | 2 周 | P0 |
| 三 | Agent 模式（沙箱 + 执行引擎 + 前端） | 2.5 周 | P0 |
| 四 | 打磨增强（UX + 安全 + 测试） | 1 周 | P1 |
| **总计** | | **~7 周** | |

---

## 新增依赖

### Rust (Cargo.toml)
```toml
reqwest = { version = "0.12", features = ["json", "stream"] }
tokio-stream = "0.1"
regex = "1"
shell-words = "1"           # Shell 命令解析
```

### 前端 (package.json)
```json
{
  "marked": "^12.0.0",       // Markdown 渲染
  "highlight.js": "^11.0.0"  // 代码高亮
}
```

---

## 新增文件结构

```
src-tauri/src/modules/ai/
├── mod.rs                 # 模块入口，注册 Tauri Commands
├── config.rs              # AI 配置管理
├── client.rs              # LLM API 客户端（流式 SSE）
├── types.rs               # 类型定义
├── chat.rs                # Chat 模式核心逻辑
├── agent.rs               # Agent 模式核心逻辑
├── sandbox.rs             # 权限沙箱
├── command_parser.rs      # Shell 命令解析器
├── context_collector.rs   # 终端上下文采集器
└── tools.rs               # Agent 可用工具定义

src/components/ai/
├── AiChatPanel.svelte     # Chat 主面板
├── ChatMessage.svelte     # 消息气泡
├── ChatInput.svelte       # 输入框
├── ChatHistory.svelte     # 对话历史
├── ContextIndicator.svelte # 上下文指示器
├── AiAgentPanel.svelte    # Agent 主面板
├── AgentToolCallStep.svelte # Agent 单步展示
├── CommandConfirmModal.svelte # 命令确认弹窗
└── SandboxIndicator.svelte # 沙箱级别指示器

src/lib/
├── aiChatService.ts       # Chat 前端服务
├── aiAgentService.ts      # Agent 前端服务
└── aiConfigService.ts     # AI 配置前端服务
```

---

## 关键设计决策

1. **LLM 接口兼容性**：采用 OpenAI API 格式作为标准，兼容大多数国内外模型服务（Claude/DeepSeek/Ollama/等），用户只需配置 base_url + api_key + model
2. **双沙箱分级**：标准沙箱（白名单放行）适用于日常使用；严格沙箱（黑名单拦截）适用于需要更灵活操作但仍有安全底线的场景
3. **终端内容传输**：通过后端 Rust 直接从 SSH 会话中提取终端缓冲区，而非前端 xterm.js 抓取 → 更可靠且不受 UI 渲染影响
4. **Agent 执行隔离**：Agent 通过现有的 SSH session 执行命令，复用已有的终端基础设施，不需要额外建立连接
5. **审计优先**：所有 Agent 操作全量记录，事后可追溯
