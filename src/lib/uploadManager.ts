import { writable, get } from 'svelte/store';

export type UploadStatus = 'uploading' | 'success' | 'error';

export interface UploadTask {
  id: string;
  fileName: string;
  targetPath: string;
  sessionId: string;
  total: number; // 总字节数（0 表示未知/空文件）
  transferred: number; // 已传输字节数
  status: UploadStatus;
  error?: string;
  startedAt: number;
}

// 全局上传任务列表，供悬浮进度卡片订阅显示
export const uploadTasks = writable<UploadTask[]>([]);

function randomTaskId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export function addUploadTask(params: {
  fileName: string;
  targetPath: string;
  sessionId: string;
  total: number;
}): string {
  const id = randomTaskId();
  const task: UploadTask = {
    id,
    fileName: params.fileName,
    targetPath: params.targetPath,
    sessionId: params.sessionId,
    total: params.total,
    transferred: 0,
    status: 'uploading',
    startedAt: Date.now(),
  };
  uploadTasks.update((tasks) => [...tasks, task]);
  return id;
}

export function updateUploadProgress(id: string, transferred: number): void {
  uploadTasks.update((tasks) =>
    tasks.map((t) => (t.id === id ? { ...t, transferred } : t))
  );
}

export function completeUploadTask(id: string): void {
  uploadTasks.update((tasks) =>
    tasks.map((t) =>
      t.id === id
        ? { ...t, status: 'success', transferred: t.total > 0 ? t.total : t.transferred }
        : t
    )
  );
}

export function failUploadTask(id: string, error: string): void {
  uploadTasks.update((tasks) =>
    tasks.map((t) => (t.id === id ? { ...t, status: 'error', error } : t))
  );
}

export function removeUploadTask(id: string): void {
  uploadTasks.update((tasks) => tasks.filter((t) => t.id !== id));
}

// 清除所有已结束（成功/失败）的任务
export function clearFinishedUploads(): void {
  uploadTasks.update((tasks) => tasks.filter((t) => t.status === 'uploading'));
}

export function hasActiveUploads(): boolean {
  return get(uploadTasks).some((t) => t.status === 'uploading');
}
