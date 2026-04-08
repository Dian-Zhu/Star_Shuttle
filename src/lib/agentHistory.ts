import type { AgentEvent, AgentStatus, AgentTaskSummary } from './aiAgentService';

export type AgentHistoryCause = 'rejected' | 'timed_out';

export interface AgentHistoryMeta {
  cause: AgentHistoryCause | null;
  causeLabel: string | null;
  preview: string | null;
}

const CAUSE_LABELS: Record<AgentHistoryCause, string> = {
  rejected: '已拒绝',
  timed_out: '超时',
};

function payloadField(payload: Record<string, unknown>, key: string): string {
  const value = payload[key];
  return value == null ? '' : String(value);
}

function statusPreview(status: AgentStatus, task: AgentTaskSummary): string | null {
  if (task.summary?.trim()) {
    return task.summary.trim();
  }

  if (task.error_message?.trim()) {
    return task.error_message.trim();
  }

  switch (status) {
    case 'completed':
      return '任务已完成';
    case 'failed':
      return '任务执行失败';
    case 'cancelled':
      return '任务已取消';
    default:
      return null;
  }
}

function latestCauseEvent(events: AgentEvent[]): AgentEvent | null {
  return (
    [...events]
      .reverse()
      .find((event) =>
        ['confirmation_timed_out', 'confirmation_rejected', 'tool_rejected_by_sandbox'].includes(
          event.event_type,
        ),
      ) ?? null
  );
}

function causePreview(event: AgentEvent): string | null {
  switch (event.event_type) {
    case 'confirmation_timed_out':
      return payloadField(event.payload_json, 'command') || '等待确认超时';
    case 'confirmation_rejected':
      return payloadField(event.payload_json, 'command') || '用户拒绝执行';
    case 'tool_rejected_by_sandbox':
      return payloadField(event.payload_json, 'reason') || '命令被沙箱拒绝';
    default:
      return null;
  }
}

export function deriveAgentHistoryMeta(
  task: AgentTaskSummary,
  events: AgentEvent[] = [],
): AgentHistoryMeta {
  const causeEvent = latestCauseEvent(events);
  const cause: AgentHistoryCause | null =
    causeEvent?.event_type === 'confirmation_timed_out'
      ? 'timed_out'
      : causeEvent
        ? 'rejected'
        : null;

  return {
    cause,
    causeLabel: cause ? CAUSE_LABELS[cause] : null,
    preview: causeEvent ? causePreview(causeEvent) : statusPreview(task.status, task),
  };
}
