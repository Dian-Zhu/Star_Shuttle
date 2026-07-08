export interface FileEntry {
  name: string;
  path: string;
  isDirectory: boolean;
  size: number;
  modified: Date;
  permissions: string;
  owner: string;
  group: string;
}

export interface CommandSnippet {
  id: string;
  name: string;
  command: string;
  description?: string;
  category?: string;
  tags?: string;
  created_at: number;
  updated_at: number;
  usage_count: number;
}

export interface CommandHistoryEntry {
  id: string;
  command: string;
  connection_id?: string | null;
  connection_name?: string | null;
  cwd?: string | null;
  executed_at: number;
}

export type TransferState = 'pending' | 'transferring' | 'paused' | 'completed' | 'failed' | 'canceled';

export interface TransferStatus {
  id: string;
  type: 'upload' | 'download';
  sessionId: string;
  localPath: string;
  remotePath: string;
  progress: number;
  speed: number;
  status: TransferState;
  startTime: Date;
  endTime?: Date;
  totalSize?: number;
  bytesTransferred: number;
  error?: string;
}
