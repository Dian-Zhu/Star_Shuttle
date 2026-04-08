# AI Agent Architecture

## 1. Layered Runtime

- **Planner** (`src-tauri/src/modules/ai/planner.rs`): the sole LLM client. It receives the planner context (session, instruction, steps, confirmation state), the active tool schemas, and emits a strong-typed `PlannerAction` (`ToolCall`, `Complete`, `Fail`).
- **Orchestrator** (`orchestrator.rs`): owns the state machine, step/confirmation lifecycle, retries, cancel handling, event emission, and final result determination. The planner never talks to tools or transitions status directly—only the orchestrator does.
- **Tool Registry** (`tools/`): centralizes `AgentTool` implementations with `name()`, `schema()`, `authorize()`, and `execute()`. Authorization handles `Allow`, `NeedConfirm`, and `Deny` decisions before execution.
- **Persisted History** (`agent_store.rs`, `ai_store.rs` tables): every task, step, and event writes to SQLite tables (`ai_agent_tasks`, `ai_agent_steps`, `ai_agent_events`), enabling history replay even after the agent stops (but not for execution recovery in this release).

## 2. Tauri Surface

The public Tauri commands sit in `src-tauri/src/modules/ai/mod.rs` and expose only the stable interface:

1. `ai_agent_start(session_id, instruction, sandbox_mode)` → returns `task_id`.
2. `ai_agent_confirm(task_id, confirmed)` → resolves pending confirmation.
3. `ai_agent_cancel(task_id)` → requests task cancellation.
4. `ai_agent_get_task(task_id)` → reads the latest `AgentTaskSnapshot` (task metadata, steps, pending confirm, final result).
5. `ai_agent_list_tasks(session_id?, limit?)` → recent task summaries.
6. `ai_agent_get_task_events(task_id)` → incremental event stream.

Events from the orchestrator are emitted via Tauri using `ai-agent-task-{task_id}` (snapshot) and `ai-agent-event-{task_id}` (new events) so the frontend can keep in sync without polling.

## 3. Status Machine

Task statuses are controlled only by the orchestrator/manager:

- `queued` → initial state before planning begins.
- `planning` → LLM is running; associated step is `planning` and `running`.
- `executing` → a tool is running.
- `waiting_confirm` → sandbox asked for confirmation; step status is `running` until user decides or times out.
- `retrying` → planner errors trigger bounded retries (max 2 retries for empty response/schema/transport, 20 max steps overall).
- `cancelling` → user requested stop; once confirmed cancelled transitions to `cancelled` terminal.
- `cancelled`/`failed`/`completed` → terminal states. `completed` requires `Complete` with a non-empty summary; any failure/empty summary/timeout sets `failed` with `error_code`/`error_message`.

Steps have their own states (`pending`, `running`, `completed`, `failed`, `rejected`, `skipped`). Confirmations use the `confirmation` kind with `running` until the user responds or times out.

Confirmation/cancel/failure behaviors:

* `NeedConfirm` (sandbox) pauses in `waiting_confirm`; the frontend emits confirm/cancel events. Timeouts and user rejects mark the step `rejected`, write `confirmation_timed_out`/`confirmation_rejected` events, and resume planning.
* `Deny` immediately records a `rejected` step with the reason and keeps the task alive.
* Cancellation writes `cancelling`, signals the orchestrator, rejects the pending confirmation (if any), and stops accepting late tool responses—events from late tools still persist but the task stays terminal.
* Tool failures emit `tool_failed` events and mark the step `failed`. Retrying is handled by the orchestrator, not the tool.

## 4. Frontend Model

The frontend consumes two parallel data streams from `src/lib/aiAgentService.ts`:

1. `activeTask` store holds the latest `AgentTaskSnapshot` (status, steps, pending confirmation, final result).
2. `activeTaskEvents` store collects the event stream for additional context (planner actions, tool completions/rejections, confirmation lifecycle).
3. `taskHistory` store lists recent task summaries for the “recent tasks” panel.

The Agent UI (`AiAgentPanel.svelte`, `AgentToolCallStep.svelte`) renders the active task snapshot plus the event stream rather than relying on a single mutable state blob. Historical lookups call `ai_agent_get_task_events`/`ai_agent_get_task` to replay past runs. There is no mechanism yet to resume execution after an app restart; history is view-only in this release.

## 5. Release Constraints

- Execution recovery after restarts, confirmation resumption, or step replay is out of scope for the first release. The history tables only support read-only replay.
- `task_complete` is not exposed as a tool. Task completion must be signaled via `Complete`.
- The sandbox stays declarative: the model can only choose tool calls; it cannot write arbitrary commands into the workflow.

---
Document maintained for cross-team clarity and should be updated whenever the Agent pipeline changes.
