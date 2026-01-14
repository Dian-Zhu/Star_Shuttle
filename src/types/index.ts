// Authentication methods
export type AuthMethod =
  | { type: 'password'; password: string; savePassword: boolean }
  | { type: 'privateKey'; keyPath: string; passphrase?: string; savePassphrase: boolean }
  | { type: 'agent'; agentPath?: string }

// Connection configuration
export interface Connection {
  id: string
  name: string
  host: string
  port: number
  username: string
  authMethod: AuthMethod
  description?: string
  tags?: string[]
  createdAt: Date
  updatedAt: Date
}

// Connection status
export type ConnectionStatus =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'disconnecting'
  | 'error'

// Session information
export interface Session {
  id: string
  connectionId: string
  status: ConnectionStatus
  terminalId?: string
  createdAt: Date
  lastActive: Date
}

// File entry
export interface FileEntry {
  name: string
  path: string
  isDirectory: boolean
  size: number
  modified: Date
  permissions: string
  owner: string
  group: string
}

// Transfer status
export interface TransferStatus {
  id: string
  type: 'upload' | 'download'
  sessionId: string
  localPath: string
  remotePath: string
  progress: number // 0-100
  speed: number // bytes per second
  status: 'pending' | 'transferring' | 'completed' | 'failed' | 'canceled'
  error?: string
  startTime: Date
  endTime?: Date
}

// Application settings
export interface AppSettings {
  general: {
    autoUpdate: boolean
    checkUpdatesOnStartup: boolean
    language: string
  }
  terminal: {
    theme: string
    fontSize: number
    fontFamily: string
    scrollbackLines: number
    cursorStyle: string
  }
  fileTransfer: {
    defaultLocalPath: string
    defaultRemotePath: string
    transferBufferSize: number
    overwriteBehavior: 'prompt' | 'overwrite' | 'skip'
    maxConcurrentTransfers: number
  }
  security: {
    savePasswords: boolean
    savePassphrases: boolean
    requireMasterPassword: boolean
    masterPasswordHint?: string
  }
}

// Log level
export type LogLevel = 'debug' | 'info' | 'warn' | 'error'

// Log entry
export interface LogEntry {
  id: string
  level: LogLevel
  message: string
  details?: any
  timestamp: Date
}
