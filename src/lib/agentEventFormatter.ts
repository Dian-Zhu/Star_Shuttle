import type { AgentEvent } from './aiAgentService';

const EVENT_LABELS: Record<string, string> = {
  task_started: '任务开始',
  task_status_changed: '状态更新',
  task_retrying: '任务重试',
  planner_action: '规划动作',
  tool_completed: '工具完成',
  tool_failed: '工具失败',
  tool_rejected_by_sandbox: '沙箱拒绝',
  confirmation_requested: '请求确认',
  confirmation_rejected: '确认被拒绝',
  confirmation_timed_out: '确认超时',
  task_completed: '任务完成',
  task_failed: '任务失败',
  task_cancelled: '任务取消',
};

function payloadField(payload: Record<string, unknown>, key: string): string {
  const value = payload[key];
  return value == null ? '' : String(value);
}

export function formatAgentEventLabel(eventType: string): string {
  return EVENT_LABELS[eventType] ?? eventType;
}

export function formatAgentEventSummary(event: Pick<AgentEvent, 'event_type' | 'payload_json'>): string {
  const payload = event.payload_json;

  switch (event.event_type) {
    case 'task_status_changed':
      return payloadField(payload, 'status');
    case 'task_retrying':
      return `第 ${payloadField(payload, 'attempt') || '?'} 次，原因：${payloadField(payload, 'reason')}`;
    case 'planner_action':
      return payloadField(payload, 'type');
    case 'tool_completed':
      return `${payloadField(payload, 'tool_name')} 已完成`;
    case 'tool_failed':
      return `${payloadField(payload, 'tool_name')} 失败：${payloadField(payload, 'error')}`;
    case 'tool_rejected_by_sandbox':
      return `${payloadField(payload, 'tool_name')} 被拒绝：${payloadField(payload, 'reason')}`;
    case 'confirmation_requested':
      return payloadField(payload, 'command');
    case 'confirmation_rejected':
      return `${payloadField(payload, 'tool_name')} 被用户拒绝`;
    case 'confirmation_timed_out':
      return `${payloadField(payload, 'tool_name')} 等待确认超时`;
    case 'task_completed':
      return payloadField(payload, 'summary');
    case 'task_failed':
      return payloadField(payload, 'error_message');
    case 'task_cancelled':
      return payloadField(payload, 'status');
    default:
      return JSON.stringify(payload);
  }
}
