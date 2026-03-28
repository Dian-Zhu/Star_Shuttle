import type { Terminal } from '@xterm/xterm';

export type ScheduledJob = { id: number; kind: 'raf' | 'timeout' };

export type IoLogger = {
  info: (component: string, message: string, ...args: any[]) => void;
  error: (
    component: string,
    message: string,
    error: unknown,
    context?: Record<string, any>
  ) => void;
  perf: (component: string, operation: string, duration: number, details?: Record<string, any>) => void;
};

type OutputWriteState = {
  chunks: string[];
  chunkIndex: number;
  scheduled: ScheduledJob | null;
  writing: boolean;
  disposed: boolean;
  chunkBudget: number;
  lastWriteTime: number;
  consecutiveSlowWrites: number;
};

type InputSendState = {
  buffer: string;
  timer: ScheduledJob | null;
  sending: boolean;
  disposed: boolean;
  lastFlushTime: number;
  pendingChunks: number;
};

const TARGET_WRITE_MS = 12;
const MIN_CHUNK_SIZE = 1024;

const outputWriteStates = new Map<string, OutputWriteState>();
const inputSendStates = new Map<string, InputSendState>();

export function nowMs(): number {
  if (typeof performance !== 'undefined' && typeof performance.now === 'function') return performance.now();
  return Date.now();
}

export function writeTerminalAsync(term: Terminal, data: string): Promise<void> {
  return new Promise((resolve, reject) => {
    try {
      term.write(data, () => resolve());
    } catch (error) {
      reject(error);
    }
  });
}

export function scheduleNext(callback: () => void): ScheduledJob {
  const hidden = typeof document !== 'undefined' && document.hidden === true;
  if (!hidden && typeof requestAnimationFrame === 'function') {
    return { id: requestAnimationFrame(callback), kind: 'raf' };
  }
  const timeout = (typeof window !== 'undefined' ? window.setTimeout : setTimeout) as typeof setTimeout;
  return { id: timeout(callback, 0) as unknown as number, kind: 'timeout' };
}

export function cancelScheduled(job: ScheduledJob | null) {
  if (!job) return;
  if (job.kind === 'raf') {
    cancelAnimationFrame(job.id);
    return;
  }
  const clear = (typeof window !== 'undefined' ? window.clearTimeout : clearTimeout) as typeof clearTimeout;
  clear(job.id as unknown as ReturnType<typeof setTimeout>);
}

function getInputSendState(sessionId: string): InputSendState {
  const existing = inputSendStates.get(sessionId);
  if (existing && !existing.disposed) return existing;
  const created: InputSendState = {
    buffer: '',
    timer: null,
    sending: false,
    disposed: false,
    lastFlushTime: 0,
    pendingChunks: 0,
  };
  inputSendStates.set(sessionId, created);
  return created;
}

function cleanupInputSendState(sessionId: string) {
  const inputState = inputSendStates.get(sessionId);
  if (!inputState) return;
  inputState.disposed = true;
  inputState.buffer = '';
  if (inputState.timer !== null) {
    cancelScheduled(inputState.timer);
  }
  inputSendStates.delete(sessionId);
}

function cleanupOutputWriteState(sessionId: string) {
  const outputState = outputWriteStates.get(sessionId);
  if (!outputState) return;
  outputState.disposed = true;
  if (outputState.scheduled !== null) {
    cancelScheduled(outputState.scheduled);
  }
  outputWriteStates.delete(sessionId);
}

export function cleanupBufferedIoState(sessionId: string) {
  cleanupOutputWriteState(sessionId);
  cleanupInputSendState(sessionId);
}

type InputBufferDeps = {
  invokeSend: (sessionId: string, data: string) => Promise<void>;
  logger: IoLogger;
};

async function flushTerminalInput(sessionId: string, state: InputSendState, deps: InputBufferDeps) {
  if (state.disposed) return;
  if (inputSendStates.get(sessionId) !== state) return;
  if (state.sending) return;
  if (!state.buffer) return;

  state.sending = true;
  const now = nowMs();
  let payload = '';

  try {
    if (state.disposed || inputSendStates.get(sessionId) !== state) return;
    const maxPayload = state.pendingChunks > 5 ? 4096 : 2048;
    payload = state.buffer.slice(0, maxPayload);
    if (!payload) return;

    state.pendingChunks++;
    await deps.invokeSend(sessionId, payload);

    if (!state.disposed && inputSendStates.get(sessionId) === state) {
      if (state.buffer.startsWith(payload)) {
        state.buffer = state.buffer.slice(payload.length);
      } else {
        state.buffer = `${payload}${state.buffer}`;
      }
    }

    state.lastFlushTime = nowMs() - now;
  } catch (error) {
    deps.logger.error('TermInput', 'Failed to send terminal data', error, {
      sessionId,
      bufferLength: state.buffer.length,
      pendingChunks: state.pendingChunks,
      payloadLength: payload.length,
    });
  } finally {
    if (state.pendingChunks > 0) {
      state.pendingChunks--;
    }
    state.sending = false;
  }

  if (state.disposed || inputSendStates.get(sessionId) !== state) return;
  if (state.buffer.length > 0 && state.timer === null) {
    state.timer = scheduleNext(() => {
      if (inputSendStates.get(sessionId) !== state || state.disposed) return;
      state.timer = null;
      void flushTerminalInput(sessionId, state, deps);
    });
  }
}

export function sendTerminalDataBuffered(
  sessionId: string,
  data: string,
  immediate: boolean,
  deps: InputBufferDeps
) {
  const state = getInputSendState(sessionId);
  if (state.disposed || inputSendStates.get(sessionId) !== state) return;
  state.buffer += data;

  const threshold = state.pendingChunks > 10 ? 2048 : 1024;
  if (state.buffer.length >= threshold) {
    if (!immediate) {
      deps.logger.info('TermInput', 'Force immediate flush', {
        bufferLength: state.buffer.length,
        threshold,
        pendingChunks: state.pendingChunks,
      });
    }
    immediate = true;
  }

  if (state.timer !== null) {
    if (!immediate) return;
    cancelScheduled(state.timer);
    state.timer = null;
  }

  if (immediate) {
    deps.logger.info('TermInput', 'Immediate flush', {
      dataLength: data.length,
      bufferLength: state.buffer.length,
    });
    void flushTerminalInput(sessionId, state, deps);
    return;
  }

  state.timer = scheduleNext(() => {
    if (inputSendStates.get(sessionId) !== state || state.disposed) return;
    state.timer = null;
    void flushTerminalInput(sessionId, state, deps);
  });
}

type OutputBufferDeps = {
  logger: IoLogger;
  isDev: boolean;
};

function getOrCreateOutputWriteState(sessionId: string): OutputWriteState {
  const existing = outputWriteStates.get(sessionId);
  if (existing && !existing.disposed) return existing;
  const created: OutputWriteState = {
    chunks: [],
    chunkIndex: 0,
    scheduled: null,
    writing: false,
    disposed: false,
    chunkBudget: 256 * 1024,
    lastWriteTime: 0,
    consecutiveSlowWrites: 0,
  };
  outputWriteStates.set(sessionId, created);
  return created;
}

async function flushOutput(
  sessionId: string,
  term: Terminal,
  state: OutputWriteState,
  deps: OutputBufferDeps
) {
  if (state.disposed) return;
  if (outputWriteStates.get(sessionId) !== state) return;
  state.scheduled = null;
  if (state.writing) return;
  if (state.chunkIndex >= state.chunks.length) return;

  if (deps.isDev && state.chunkIndex === 0) {
    deps.logger.info('TermOutput', 'Starting flush', {
      sessionId,
      totalChunks: state.chunks.length,
      chunkBudget: state.chunkBudget,
    });
  }

  const chunkLimit = state.chunkBudget;
  let count = 0;
  const parts: string[] = [];

  while (state.chunkIndex < state.chunks.length) {
    const nextChunk = state.chunks[state.chunkIndex];
    const wouldExceed = count + nextChunk.length > chunkLimit && count > 0;
    if (wouldExceed && count >= MIN_CHUNK_SIZE) break;

    parts.push(nextChunk);
    count += nextChunk.length;
    state.chunkIndex += 1;

    if (count >= chunkLimit) break;
  }

  if (state.chunks.length > 4096 || state.chunkIndex > Math.floor(state.chunks.length * 0.7)) {
    state.chunks.splice(0, state.chunkIndex);
    state.chunkIndex = 0;
  }

  const payload = parts.join('').split('\u0000').join('');
  if (payload.length === 0) return;

  if (deps.isDev && state.chunkIndex <= 2) {
    deps.logger.info('TermOutput', 'Writing to terminal', {
      sessionId,
      payloadLength: payload.length,
      payloadPreview: payload.substring(0, 50),
    });
  }

  state.writing = true;
  const writeStart = nowMs();
  try {
    await writeTerminalAsync(term, payload);
    const writeDuration = nowMs() - writeStart;
    state.lastWriteTime = writeDuration;

    if (writeDuration > TARGET_WRITE_MS) {
      state.consecutiveSlowWrites++;
      const reductionFactor = 0.5 + 0.2 / Math.max(1, state.consecutiveSlowWrites);
      state.chunkBudget = Math.max(32 * 1024, Math.floor(state.chunkBudget * reductionFactor));

      deps.logger.perf('TermOutput', 'write', writeDuration, {
        sessionId,
        payloadLength: payload.length,
        pendingChunks: state.chunks.length - state.chunkIndex,
        budget: state.chunkBudget,
        consecutiveSlowWrites: state.consecutiveSlowWrites,
      });
    } else if (writeDuration < TARGET_WRITE_MS / 2) {
      if (state.consecutiveSlowWrites > 0) {
        state.consecutiveSlowWrites = Math.max(0, state.consecutiveSlowWrites - 1);
      }
      state.chunkBudget = Math.min(2 * 1024 * 1024, Math.floor(state.chunkBudget * 1.1));
    }

    state.writing = false;
    if (state.chunkIndex < state.chunks.length && !state.disposed) {
      scheduleOutputFlush(sessionId, term, state, deps);
    }
  } catch (error) {
    deps.logger.error('TermOutput', `write failed for session ${sessionId}`, error, {
      payloadLength: payload.length,
      pendingChunks: state.chunks.length - state.chunkIndex,
      budget: state.chunkBudget,
    });
    state.writing = false;
    const retryDelay = Math.min(100, state.consecutiveSlowWrites * 10);
    state.scheduled = {
      id: setTimeout(() => scheduleOutputFlush(sessionId, term, state, deps), retryDelay) as unknown as number,
      kind: 'timeout',
    };
  }
}

function scheduleOutputFlush(
  sessionId: string,
  term: Terminal,
  state: OutputWriteState,
  deps: OutputBufferDeps
) {
  if (state.disposed) return;
  if (outputWriteStates.get(sessionId) !== state) return;
  if (state.scheduled !== null) return;
  state.scheduled = scheduleNext(() => {
    void flushOutput(sessionId, term, state, deps);
  });
}

export function initializeOutputBuffer(sessionId: string) {
  cleanupOutputWriteState(sessionId);
  getOrCreateOutputWriteState(sessionId);
}

export function disposeOutputBuffer(sessionId: string) {
  cleanupOutputWriteState(sessionId);
}

export function enqueueTerminalOutput(
  sessionId: string,
  term: Terminal,
  data: string,
  deps: OutputBufferDeps
) {
  const state = getOrCreateOutputWriteState(sessionId);
  if (state.disposed || outputWriteStates.get(sessionId) !== state) return;
  state.chunks.push(data);
  if (deps.isDev && state.chunks.length <= 5) {
    deps.logger.info('TermOutput', 'Chunk added', {
      sessionId,
      chunkIndex: state.chunks.length - 1,
      isWriting: state.writing,
    });
  }
  if (!state.writing) {
    scheduleOutputFlush(sessionId, term, state, deps);
  }
}

export const __terminalIoBufferTestHooks = {
  cleanupAll() {
    for (const key of outputWriteStates.keys()) {
      cleanupOutputWriteState(key);
    }
    for (const key of inputSendStates.keys()) {
      cleanupInputSendState(key);
    }
  },
};
