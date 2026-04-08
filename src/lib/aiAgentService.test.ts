import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: mocks.listen,
}));

import { activeTask, activeTaskEvents, cleanup, startAgent, taskHistory } from './aiAgentService';

describe('aiAgentService skill command payload', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    cleanup();
    taskHistory.set([]);
    activeTask.set(null);
    activeTaskEvents.set([]);
    mocks.listen.mockResolvedValue(() => {});
    mocks.invoke.mockImplementation(async (command: string) => {
      switch (command) {
        case 'ai_agent_start':
          return 'task-1';
        case 'ai_agent_get_task':
          return {
            id: 'task-1',
            session_id: 'session-1',
            instruction: 'check docker',
            skill_id: 'docker_troubleshooting',
            sandbox_mode: 'standard',
            status: 'queued',
            steps: [],
            pending_confirm: null,
            final_result: null,
            summary: null,
            error_code: null,
            error_message: null,
            started_at: '2026-04-08T00:00:00Z',
            finished_at: null,
          };
        case 'ai_agent_get_task_events':
          return [];
        case 'ai_agent_list_tasks':
          return [];
        default:
          throw new Error(`unexpected command: ${command}`);
      }
    });
  });

  it('passes skillId to ai_agent_start', async () => {
    await startAgent('session-1', 'check docker', 'standard', 'docker_troubleshooting');

    expect(mocks.invoke).toHaveBeenCalledWith('ai_agent_start', {
      sessionId: 'session-1',
      instruction: 'check docker',
      sandboxMode: 'standard',
      skillId: 'docker_troubleshooting',
    });
  });
});
