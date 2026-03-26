import { get } from 'svelte/store';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  sftpWriteFile: vi.fn(),
  sftpReadChunk: vi.fn(),
  sftpScpDownload: vi.fn(),
  localGetFileSize: vi.fn(),
  localOpenFile: vi.fn(),
  localReadChunk: vi.fn(),
  localCloseFile: vi.fn(),
  localOpenWriteFile: vi.fn(),
  localWriteChunk: vi.fn(),
}));

vi.mock('./sftpService', () => ({
  sftpService: {
    writeFile: mocks.sftpWriteFile,
    readChunk: mocks.sftpReadChunk,
    scpDownload: mocks.sftpScpDownload,
  },
}));

vi.mock('./localFsService', () => ({
  localFsService: {
    getFileSize: mocks.localGetFileSize,
    openFile: mocks.localOpenFile,
    readChunk: mocks.localReadChunk,
    closeFile: mocks.localCloseFile,
    openWriteFile: mocks.localOpenWriteFile,
    writeChunk: mocks.localWriteChunk,
  },
}));

async function loadModule() {
  vi.resetModules();
  return await import('./transferQueueService');
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>(res => {
    resolve = res;
  });
  return { promise, resolve };
}

describe('TransferQueueService queue progression', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();

    mocks.localGetFileSize.mockResolvedValue(1);
    mocks.localOpenFile.mockImplementation(async () => ({ reads: 0 }));
    mocks.localReadChunk.mockImplementation(async (handle: { reads: number }) => {
      if (handle.reads === 0) {
        handle.reads += 1;
        return new Uint8Array([1]);
      }
      return new Uint8Array([]);
    });
    mocks.localCloseFile.mockResolvedValue(undefined);
    mocks.sftpWriteFile.mockResolvedValue(undefined);
  });

  it('starts pending transfers beyond concurrency once earlier ones complete', async () => {
    const { TransferQueueService } = await loadModule();
    const service = new TransferQueueService();

    await service.addTransfer('upload', 's1', '/tmp/a', '/r/a', 1);
    await service.addTransfer('upload', 's1', '/tmp/b', '/r/b', 1);
    await service.addTransfer('upload', 's1', '/tmp/c', '/r/c', 1);
    await service.addTransfer('upload', 's1', '/tmp/d', '/r/d', 1);

    await vi.runAllTimersAsync();

    expect(mocks.sftpWriteFile).toHaveBeenCalledTimes(4);
  });

  it('cleans completed transfers and leaves no pending queue items', async () => {
    const { TransferQueueService, transfers } = await loadModule();
    const service = new TransferQueueService();

    await service.addTransfer('upload', 's1', '/tmp/single', '/r/single', 1);
    await vi.runAllTimersAsync();

    const snapshot = get(transfers);
    expect(snapshot.active).toHaveLength(0);
    expect(snapshot.queue).toHaveLength(0);
    expect(mocks.sftpWriteFile).toHaveBeenCalledTimes(1);
  });

  it('does not start a second executeTransfer for same id during quick pause/resume', async () => {
    const readGate = deferred<Uint8Array>();
    let readCalls = 0;

    mocks.localGetFileSize.mockResolvedValue(2);
    mocks.localOpenFile.mockImplementation(async () => ({
      seek: vi.fn(async () => undefined),
    }));
    mocks.localReadChunk.mockImplementation(async () => {
      readCalls += 1;
      if (readCalls === 1) return new Uint8Array([1]);
      if (readCalls === 2) return readGate.promise;
      return new Uint8Array([]);
    });

    const { TransferQueueService } = await loadModule();
    const service = new TransferQueueService();

    const id = await service.addTransfer('upload', 's1', '/tmp/fast', '/r/fast', 2);
    await vi.runAllTimersAsync();

    service.pauseTransfer(id);
    service.resumeTransfer(id);

    readGate.resolve(new Uint8Array([2]));
    await vi.runAllTimersAsync();

    expect(mocks.localOpenFile).toHaveBeenCalledTimes(1);
    expect(mocks.sftpWriteFile).toHaveBeenCalledTimes(2);
  });

  it('pauses cleanly and resumes from previous offset with append mode', async () => {
    const secondReadGate = deferred<Uint8Array>();
    let readCalls = 0;

    mocks.localGetFileSize.mockResolvedValue(2);
    mocks.localOpenFile.mockImplementation(async () => {
      const handle = {
        position: 0,
        async seek(offset: number) {
          handle.position = offset;
        },
      };
      return handle;
    });
    mocks.localReadChunk.mockImplementation(async (handle: { position: number }) => {
      readCalls += 1;
      if (readCalls === 1) {
        handle.position += 1;
        return new Uint8Array([1]);
      }
      if (readCalls === 2) {
        const chunk = await secondReadGate.promise;
        handle.position += chunk.length;
        return chunk;
      }
      if (handle.position >= 2) return new Uint8Array([]);
      handle.position += 1;
      return new Uint8Array([3]);
    });

    const { TransferQueueService, transfers } = await loadModule();
    const service = new TransferQueueService();

    const id = await service.addTransfer('upload', 's1', '/tmp/pause', '/r/pause', 2);
    await vi.runAllTimersAsync();

    service.pauseTransfer(id);
    secondReadGate.resolve(new Uint8Array([2]));
    await vi.runAllTimersAsync();

    let snapshot = get(transfers);
    expect(snapshot.active).toHaveLength(0);
    expect(snapshot.queue.find(t => t.id === id)?.status).toBe('paused');
    expect(mocks.sftpWriteFile).toHaveBeenCalledTimes(1);

    service.resumeTransfer(id);
    await vi.runAllTimersAsync();

    snapshot = get(transfers);
    expect(snapshot.active).toHaveLength(0);
    expect(snapshot.queue).toHaveLength(0);
    expect(mocks.localOpenFile).toHaveBeenCalledTimes(2);
    expect(mocks.sftpWriteFile).toHaveBeenCalledTimes(2);
    expect(mocks.sftpWriteFile.mock.calls[1][3]).toBe(true);
  });
});
