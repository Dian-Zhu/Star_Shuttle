import { describe, expect, it } from 'vitest';
import { formatAgentEventLabel, formatAgentEventSummary } from './agentEventFormatter';

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

  it('falls back to raw json for unknown events', () => {
    expect(
      formatAgentEventSummary({
        event_type: 'custom_event',
        payload_json: { foo: 'bar' },
      }),
    ).toBe('{"foo":"bar"}');
  });
});
