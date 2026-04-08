import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { writable } from 'svelte/store';

export type AgentStatus =
  | 'queued'
  | 'planning'
  | 'executing'
  | 'waiting_confirm'
  | 'retrying'
  | 'cancelling'
  | 'completed'
  | 'failed'
  | 'cancelled';

export type StepStatus = 'pending' | 'running' | 'completed' | 'failed' | 'rejected' | 'skipped';

export type StepKind = 'planning' | 'tool_execution' | 'confirmation' | 'result';

export interface AgentStep {
  id: string;
  seq: number;
  kind: StepKind;
  title: string;
  tool_name: string | null;
  command: string | null;
  output: string | null;
  status: StepStatus;
  risk_level: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface PendingConfirm {
  task_id: string;
  step_id: string;
  command: string;
  reason: string;
  risk_level: string;
}

export interface AgentFinalResult {
  status: AgentStatus;
  summary: string | null;
  error_code: string | null;
  error_message: string | null;
  last_successful_step_id: string | null;
}

export interface AgentTaskSnapshot {
  id: string;
  session_id: string;
  instruction: string;
  sandbox_mode: 'standard' | 'full';
  status: AgentStatus;
  steps: AgentStep[];
  pending_confirm: PendingConfirm | null;
  final_result: AgentFinalResult | null;
  summary: string | null;
  error_code: string | null;
  error_message: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface AgentTaskSummary {
  id: string;
  session_id: string;
  instruction: string;
  sandbox_mode: 'standard' | 'full';
  status: AgentStatus;
  summary: string | null;
  error_code: string | null;
  error_message: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface AgentEvent {
  id: string;
  task_id: string;
  seq: number;
  event_type: string;
  payload_json: Record<string, unknown>;
  created_at: string;
}

export const activeTask = writable<AgentTaskSnapshot | null>(null);
export const activeTaskEvents = writable<AgentEvent[]>([]);
export const taskHistory = writable<AgentTaskSummary[]>([]);
export const sandboxMode = writable<'standard' | 'full'>('standard');
export const pendingConfirm = writable<PendingConfirm | null>(null);

let taskUnsub: UnlistenFn | null = null;
let eventUnsub: UnlistenFn | null = null;
let currentSessionId: string | null = null;

function upsertHistoryItem(task: AgentTaskSnapshot) {
  taskHistory.update((tasks) => {
    const next = [...tasks];
    const summary = {
      id: task.id,
      session_id: task.session_id,
      instruction: task.instruction,
      sandbox_mode: task.sandbox_mode,
      status: task.status,
      summary: task.summary,
      error_code: task.error_code,
      error_message: task.error_message,
      started_at: task.started_at,
      finished_at: task.finished_at,
    } satisfies AgentTaskSummary;
    const index = next.findIndex((item) => item.id === task.id);
    if (index >= 0) {
      next[index] = summary;
    } else {
      next.unshift(summary);
    }
    return next.sort((a, b) => (a.started_at < b.started_at ? 1 : -1));
  });
}

async function subscribeToTask(taskId: string) {
  taskUnsub?.();
  eventUnsub?.();

  taskUnsub = await listen<AgentTaskSnapshot>(`ai-agent-task-${taskId}`, (ev) => {
    activeTask.set(ev.payload);
    pendingConfirm.set(ev.payload.pending_confirm ?? null);
    upsertHistoryItem(ev.payload);
  });

  eventUnsub = await listen<AgentEvent>(`ai-agent-event-${taskId}`, (ev) => {
    activeTaskEvents.update((events) => [...events, ev.payload].sort((a, b) => a.seq - b.seq));
  });
}

export async function loadTaskHistory(sessionId?: string | null): Promise<void> {
  currentSessionId = sessionId ?? currentSessionId;
  const tasks = await invoke<AgentTaskSummary[]>('ai_agent_list_tasks', {
    sessionId: currentSessionId ?? undefined,
    limit: 20,
  });
  taskHistory.set(tasks);
}

export async function openTask(taskId: string): Promise<void> {
  await subscribeToTask(taskId);
  const [task, events] = await Promise.all([
    invoke<AgentTaskSnapshot | null>('ai_agent_get_task', { taskId }),
    invoke<AgentEvent[]>('ai_agent_get_task_events', { taskId }),
  ]);

  activeTask.set(task);
  activeTaskEvents.set(events);
  pendingConfirm.set(task?.pending_confirm ?? null);
  if (task) {
    upsertHistoryItem(task);
  }
}

export async function startAgent(
  sessionId: string,
  instruction: string,
  mode: 'standard' | 'full',
): Promise<string> {
  currentSessionId = sessionId;
  const taskId = await invoke<string>('ai_agent_start', {
    sessionId,
    instruction,
    sandboxMode: mode,
  });
  await openTask(taskId);
  await loadTaskHistory(sessionId);
  return taskId;
}

export async function confirmStep(taskId: string, confirmed: boolean): Promise<void> {
  await invoke('ai_agent_confirm', { taskId, confirmed });
  pendingConfirm.set(null);
}

export async function cancelTask(taskId: string): Promise<void> {
  await invoke('ai_agent_cancel', { taskId });
}

export function cleanup() {
  taskUnsub?.();
  eventUnsub?.();
  taskUnsub = null;
  eventUnsub = null;
  activeTask.set(null);
  activeTaskEvents.set([]);
  pendingConfirm.set(null);
}
