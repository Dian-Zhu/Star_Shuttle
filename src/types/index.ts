// Authentication methods
export type AuthMethod =
  | { type: 'password'; password: string; savePassword: boolean }

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}
  | { type: 'privateKey'; keyPath: string; passphrase?: string; savePassphrase: boolean }

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}
  | { type: 'agent'; agentPath?: string }

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}
  | { type: 'certificate'; certificatePath: string; privateKeyPath: string; passphrase?: string; savePassphrase: boolean }

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}

// Proxy types for jump host support
export type ProxyType =
  | { type: 'none' }

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}
  | { type: 'socks5'; host: string; port: number; username?: string; password?: string }

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}
  | { type: 'http'; host: string; port: number; username?: string; password?: string }

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}
  | { type: 'jumpHost'; host: string; port: number; username: string; authMethod: AuthMethod }

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}

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
  proxyType: ProxyType
  socksProxyPort?: number
  localForwards?: Array<{ localHost: string; localPort: number; remoteHost: string; remotePort: number }

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}>
  remoteForwards?: Array<{ remoteHost: string; remotePort: number; localHost: string; localPort: number }

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}>
}

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
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

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
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

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
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
  status: 'pending' | 'transferring' | 'paused' | 'completed' | 'failed' | 'canceled'
  error?: string
  startTime: Date
  endTime?: Date
  // For pause/resume support
  bytesTransferred?: number
  totalSize?: number
}

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}

// Application settings
export interface AppSettings {
  general: {
    autoUpdate: boolean
    checkUpdatesOnStartup: boolean
    language: string
  }

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}
  terminal: {
    theme: string
    fontSize: number
    fontFamily: string
    scrollbackLines: number
    cursorStyle: string
  }

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}
  fileTransfer: {
    defaultLocalPath: string
    defaultRemotePath: string
    transferBufferSize: number
    overwriteBehavior: 'prompt' | 'overwrite' | 'skip'
    maxConcurrentTransfers: number
  }

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}
  security: {
    savePasswords: boolean
    savePassphrases: boolean
    requireMasterPassword: boolean
    masterPasswordHint?: string
  }

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}
}

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
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

// Command snippet for quick command library
export interface CommandSnippet {
  id: string
  name: string
  command: string
  description?: string
  category?: string
  tags?: string[]
  created_at: number
  updated_at: number
  usage_count: number
}
