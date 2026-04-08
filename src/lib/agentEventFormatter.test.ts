import { describe, expect, it } from 'vitest';
import {
  formatAgentEventLabel,
  formatAgentEventSummary,
  formatAgentEventTone,
} from './agentEventFormatter';

describe('agentEventFormatter', () => {
  it('formats known labels', () => {
    expect(formatAgentEventLabel('task_started')).toBe('任务开始');
    expect(formatAgentEventLabel('unknown_event')).toBe('unknown_event');
  });

  it('formats retry summary', () => {
    expect(
      formatAgentEventSummary({
        event_type: 'task_retrying',
        payload_json: { attempt: 2, reason: '模型返回空响应' },
      }),
    ).toBe('第 2 次，原因：模型返回空响应');
  });

  it('formats tool failure summary', () => {
    expect(
      formatAgentEventSummary({
        event_type: 'tool_failed',
        payload_json: { tool_name: 'execute_command', error: 'permission denied' },
      }),
    ).toBe('execute_command 失败：permission denied');
  });

  it('formats confirmation timeout summary', () => {
    expect(
      formatAgentEventSummary({
        event_type: 'confirmation_timed_out',
        payload_json: { command: 'systemctl restart nginx' },
      }),
    ).toBe('等待确认超时：systemctl restart nginx');
  });

  it('formats task failure code and tone', () => {
    expect(
      formatAgentEventSummary({
        event_type: 'task_failed',
        payload_json: { error_code: 'planner_failed', error_message: '模型拒绝继续' },
      }),
    ).toBe('planner_failed：模型拒绝继续');
    expect(formatAgentEventTone('task_failed')).toBe('danger');
    expect(formatAgentEventTone('task_completed')).toBe('success');
  });

  it('falls back to raw json for unknown events', () => {
    expect(
      formatAgentEventSummary({
        event_type: 'custom_event',
        payload_json: { foo: 'bar' },
      }),
    ).toBe('{"foo":"bar"}');
  });
});
