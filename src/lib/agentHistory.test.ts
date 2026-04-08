import { describe, expect, it } from 'vitest';
import { deriveAgentHistoryMeta } from './agentHistory';
import type { AgentEvent, AgentTaskSummary } from './aiAgentService';

function buildTask(overrides: Partial<AgentTaskSummary> = {}): AgentTaskSummary {
  return {
    id: 'task-1',
    session_id: 'session-1',
    instruction: 'check nginx',
    skill_id: null,
    sandbox_mode: 'standard',
    status: 'completed',
    summary: 'nginx is healthy',
    error_code: null,
    error_message: null,
    started_at: '2026-04-08T10:00:00Z',
    finished_at: '2026-04-08T10:01:00Z',
    ...overrides,
  };
}

function buildEvent(
  event_type: AgentEvent['event_type'],
  payload_json: AgentEvent['payload_json'],
  seq: number,
): AgentEvent {
  return {
    id: `event-${seq}`,
    task_id: 'task-1',
    seq,
    event_type,
    payload_json,
    created_at: `2026-04-08T10:00:0${seq}Z`,
  };
}

describe('agentHistory', () => {
  it('uses terminal task summary by default', () => {
    expect(deriveAgentHistoryMeta(buildTask())).toEqual({
      cause: null,
      causeLabel: null,
      preview: 'nginx is healthy',
    });
  });

  it('surfaces timeout cause for replay history', () => {
    expect(
      deriveAgentHistoryMeta(buildTask({ status: 'failed', summary: null }), [
        buildEvent('confirmation_requested', { command: 'systemctl restart nginx' }, 1),
        buildEvent('confirmation_timed_out', { command: 'systemctl restart nginx' }, 2),
      ]),
    ).toEqual({
      cause: 'timed_out',
      causeLabel: '超时',
      preview: 'systemctl restart nginx',
    });
  });

  it('surfaces rejection cause for replay history', () => {
    expect(
      deriveAgentHistoryMeta(buildTask({ status: 'failed', summary: null }), [
        buildEvent('tool_rejected_by_sandbox', { reason: '危险命令' }, 1),
      ]),
    ).toEqual({
      cause: 'rejected',
      causeLabel: '已拒绝',
      preview: '危险命令',
    });
  });
});
