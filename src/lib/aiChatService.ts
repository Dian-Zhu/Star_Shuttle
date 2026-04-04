import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { writable, get } from 'svelte/store';

// ── Types ─────────────────────────────────────────────────────────────────────

export interface Conversation {
  id: string;
  title: string;
  session_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface StoredMessage {
  id: string;
  conversation_id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  context_snapshot: string | null;
  created_at: string;
}

export type StreamEventType =
  | { type: 'delta'; content: string }
  | { type: 'done'; conversation_id: string }
  | { type: 'error'; message: string };

// ── Stores ────────────────────────────────────────────────────────────────────

export const conversations = writable<Conversation[]>([]);
export const activeConversationId = writable<string | null>(null);
export const messages = writable<StoredMessage[]>([]);
export const isSending = writable(false);
export const streamingContent = writable('');   // content accumulating during stream
export const sendingConversationId = writable<string | null>(null);
export const pendingContextSnapshot = writable<string | null>(null);

// ── API Calls ─────────────────────────────────────────────────────────────────

export async function loadConversations(): Promise<void> {
  const list = await invoke<Conversation[]>('ai_chat_list');
  conversations.set(list);
}

export async function createConversation(sessionId?: string): Promise<string> {
  const id = await invoke<string>('ai_chat_new', {
    sessionId: sessionId ?? null,
  });
  await loadConversations();
  return id;
}

export async function loadMessages(conversationId: string): Promise<void> {
  const msgs = await invoke<StoredMessage[]>('ai_chat_messages', { conversationId });
  messages.set(msgs);
  activeConversationId.set(conversationId);
}

export async function deleteConversation(conversationId: string): Promise<void> {
  await invoke('ai_chat_delete', { conversationId });
  await loadConversations();
  if (get(activeConversationId) === conversationId) {
    activeConversationId.set(null);
    messages.set([]);
  }
}

export async function clearMessages(conversationId: string): Promise<void> {
  await invoke('ai_chat_clear', { conversationId });
  if (get(activeConversationId) === conversationId) {
    messages.set([]);
  }
}

// ── Send Message (streaming) ──────────────────────────────────────────────────

/**
 * Send a message and stream the response back.
 * @param content        User message text
 * @param conversationId Target conversation
 * @param sessionId      SSH session to pull terminal context from (optional)
 * @param includeContext Whether to attach terminal context
 * @param onDelta        Called for each streamed token
 */
export async function sendMessage(
  content: string,
  conversationId: string,
  sessionId: string | null,
  includeContext: boolean,
  onDelta?: (delta: string) => void,
): Promise<void> {
  isSending.set(true);
  sendingConversationId.set(conversationId);
  streamingContent.set('');

  try {
    let contextSnapshot: string | null = null;
    if (includeContext && sessionId) {
      try {
        const context = await invoke<{ content: string }>('ai_get_terminal_context', {
          sessionId,
          lines: 100,
        });
        contextSnapshot = context.content;
        pendingContextSnapshot.set(contextSnapshot);
      } catch {
        contextSnapshot = null;
        pendingContextSnapshot.set(null);
      }
    } else {
      pendingContextSnapshot.set(null);
    }

    // Optimistically add user message to UI
    const tempUserMsg: StoredMessage = {
      id: crypto.randomUUID(),
      conversation_id: conversationId,
      role: 'user',
      content,
      context_snapshot: contextSnapshot,
      created_at: new Date().toISOString(),
    };

    messages.update(m => [...m, tempUserMsg]);

    // Placeholder for AI reply during streaming
    const tempAiId = crypto.randomUUID();
    const tempAiMsg: StoredMessage = {
      id: tempAiId,
      conversation_id: conversationId,
      role: 'assistant',
      content: '',
      context_snapshot: null,
      created_at: new Date().toISOString(),
    };
    messages.update(m => [...m, tempAiMsg]);

    // Listen for stream events BEFORE invoking to avoid race condition
    const eventName = `ai-chat-stream-${conversationId}`;
    let unlisten: UnlistenFn | null = null;

    try {
      unlisten = await listen<StreamEventType>(eventName, (ev) => {
        const event = ev.payload;
        if (event.type === 'delta') {
          streamingContent.update(c => c + event.content);
          onDelta?.(event.content);
          // Update the streaming placeholder message in real time
          messages.update(m =>
            m.map(msg =>
              msg.id === tempAiId
                ? { ...msg, content: msg.content + event.content }
                : msg,
            ),
          );
        }
      });

      await invoke('ai_chat_send', {
        conversationId,
        content,
        sessionId: sessionId ?? null,
        includeTerminalContext: includeContext,
      });

      // After completion, reload messages from DB to get the real IDs
      const finalMsgs = await invoke<StoredMessage[]>('ai_chat_messages', { conversationId });
      messages.set(finalMsgs);

      // Refresh conversation list (title may have been set)
      await loadConversations();
    } catch (err) {
      // On error, replace streaming placeholder with error message
      messages.update(m =>
        m.map(msg =>
          msg.id === tempAiId
            ? { ...msg, content: `(Error: ${err})` }
            : msg,
        ),
      );
      throw err;
    } finally {
      unlisten?.();
    }
  } finally {
    isSending.set(false);
    sendingConversationId.set(null);
    streamingContent.set('');
    pendingContextSnapshot.set(null);
  }
}

export async function cancelMessage(conversationId: string): Promise<void> {
  await invoke('ai_chat_cancel', { conversationId });
}

