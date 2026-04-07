import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { writable } from 'svelte/store';

// ── Types ─────────────────────────────────────────────────────────────────────

export type AgentStatus =
  | 'running'
  | 'waiting_confirm'
  | 'completed'
  | 'failed'
  | 'cancelled';

export type StepStatus =
  | 'pending'
  | 'running'
  | 'waiting_confirm'
  | 'confirmed'
  | 'rejected'
  | 'completed'
  | 'failed';

export type StepKind =
  | 'thinking'
  | 'execute_command'
  | 'read_file'
  | 'list_directory'
  | 'get_system_info'
  | 'awaiting_confirm'
  | 'result';

export interface AgentStep {
  id: string;
  kind: StepKind;
  description: string;
  command: string | null;
  output: string | null;
  status: StepStatus;
  risk_level: string | null;
}

export interface PendingConfirm {
  task_id: string;
  step_id: string;
  command: string;
  reason: string;
  risk_level: string;
}

export interface AgentTask {
  id: string;
  session_id: string;
  instruction: string;
  sandbox_mode: 'standard' | 'full';
  status: AgentStatus;
  steps: AgentStep[];
  pending_confirm: PendingConfirm | null;
  error: string | null;
}

// ── Stores ────────────────────────────────────────────────────────────────────

export const currentTask = writable<AgentTask | null>(null);
export const sandboxMode = writable<'standard' | 'full'>('standard');
export const pendingConfirm = writable<PendingConfirm | null>(null);

// ── Task Lifecycle ────────────────────────────────────────────────────────────

let statusUnsub: UnlistenFn | null = null;
let confirmUnsub: UnlistenFn | null = null;

export async function startAgent(
  sessionId: string,
  instruction: string,
  mode: 'standard' | 'full',
): Promise<string> {
  // Clean up previous listeners
  statusUnsub?.();
  confirmUnsub?.();

  const taskId = await invoke<string>('ai_agent_start', {
    sessionId,
    instruction,
    sandboxMode: mode,
  });

  // Listen for status updates
  statusUnsub = await listen<AgentTask>(`ai-agent-status-${taskId}`, (ev) => {
    currentTask.set(ev.payload);
    if (ev.payload.pending_confirm) {
      pendingConfirm.set(ev.payload.pending_confirm);
    } else {
      pendingConfirm.set(null);
    }
  });

  // Global confirm request (fallback, in case listener attaches after event)
  confirmUnsub = await listen<PendingConfirm>('ai-agent-confirm-request', (ev) => {
    pendingConfirm.set(ev.payload);
  });

  return taskId;
}

export async function confirmStep(taskId: string, confirmed: boolean): Promise<void> {
  await invoke('ai_agent_confirm', { taskId, confirmed });
  pendingConfirm.set(null);
}

export async function cancelTask(taskId: string): Promise<void> {
  await invoke('ai_agent_cancel', { taskId });
}

export async function getTaskStatus(taskId: string): Promise<AgentTask | null> {
  return invoke<AgentTask | null>('ai_agent_status', { taskId });
}

export function cleanup() {
  statusUnsub?.();
  confirmUnsub?.();
  statusUnsub = null;
  confirmUnsub = null;
  currentTask.set(null);
  pendingConfirm.set(null);
}
