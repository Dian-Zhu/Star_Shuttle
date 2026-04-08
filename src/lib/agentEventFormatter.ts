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

const EVENT_TONES: Record<string, 'neutral' | 'success' | 'warning' | 'danger'> = {
  task_started: 'neutral',
  task_status_changed: 'neutral',
  task_retrying: 'warning',
  planner_action: 'neutral',
  tool_completed: 'success',
  tool_failed: 'danger',
  tool_rejected_by_sandbox: 'danger',
  confirmation_requested: 'warning',
  confirmation_rejected: 'danger',
  confirmation_timed_out: 'warning',
  task_completed: 'success',
  task_failed: 'danger',
  task_cancelled: 'neutral',
};

function payloadField(payload: Record<string, unknown>, key: string): string {
  const value = payload[key];
  return value == null ? '' : String(value);
}

export function formatAgentEventLabel(eventType: string): string {
  return EVENT_LABELS[eventType] ?? eventType;
}

export function formatAgentEventTone(
  eventType: string,
): 'neutral' | 'success' | 'warning' | 'danger' {
  return EVENT_TONES[eventType] ?? 'neutral';
}

export function formatAgentEventSummary(event: Pick<AgentEvent, 'event_type' | 'payload_json'>): string {
  const payload = event.payload_json;

  switch (event.event_type) {
    case 'task_status_changed':
      return payloadField(payload, 'status');
    case 'task_retrying':
      return `第 ${payloadField(payload, 'attempt') || '?'} 次，原因：${payloadField(payload, 'reason')}`;
    case 'planner_action':
      if (payloadField(payload, 'type') === 'tool_call') {
        return `准备调用 ${payloadField(payload, 'tool_name') || '工具'}`;
      }
      if (payloadField(payload, 'type') === 'complete') {
        return '准备结束任务';
      }
      if (payloadField(payload, 'type') === 'fail') {
        return `准备报告失败：${payloadField(payload, 'reason')}`;
      }
      return payloadField(payload, 'type');
    case 'tool_completed':
      return `${payloadField(payload, 'tool_name')} 已完成：${payloadField(payload, 'command') || payloadField(payload, 'title')}`;
    case 'tool_failed':
      return `${payloadField(payload, 'tool_name')} 失败：${payloadField(payload, 'error')}`;
    case 'tool_rejected_by_sandbox':
      return `${payloadField(payload, 'tool_name')} 被拒绝：${payloadField(payload, 'reason')}`;
    case 'confirmation_requested':
      return `${payloadField(payload, 'command')} (${payloadField(payload, 'reason')})`;
    case 'confirmation_rejected':
      return `用户拒绝执行：${payloadField(payload, 'command') || payloadField(payload, 'tool_name')}`;
    case 'confirmation_timed_out':
      return `等待确认超时：${payloadField(payload, 'command') || payloadField(payload, 'tool_name')}`;
    case 'task_completed':
      return payloadField(payload, 'summary');
    case 'task_failed':
      return `${payloadField(payload, 'error_code') || 'failed'}：${payloadField(payload, 'error_message')}`;
    case 'task_cancelled':
      return '任务已取消';
    default:
      return JSON.stringify(payload);
  }
}
