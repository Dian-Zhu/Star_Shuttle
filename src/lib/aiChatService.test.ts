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

import { messages, sendMessage, streamingContent } from './aiChatService';

describe('aiChatService skill command payload', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    messages.set([]);
    streamingContent.set('');
    mocks.listen.mockResolvedValue(() => {});
    mocks.invoke.mockImplementation(async (command: string) => {
      switch (command) {
        case 'ai_chat_send':
          return '';
        case 'ai_chat_messages':
          return [];
        case 'ai_chat_list':
          return [];
        default:
          throw new Error(`unexpected command: ${command}`);
      }
    });
  });

  it('passes skillId to ai_chat_send', async () => {
    await sendMessage('check logs', 'conv-1', null, false, 'log_diagnostics');

    expect(mocks.invoke).toHaveBeenCalledWith('ai_chat_send', {
      conversationId: 'conv-1',
      content: 'check logs',
      sessionId: null,
      includeTerminalContext: false,
      skillId: 'log_diagnostics',
    });
  });
});
