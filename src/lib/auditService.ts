import { invoke } from '@tauri-apps/api/core';

const RISK_ORDER = { LOW: 0, MEDIUM: 1, HIGH: 2, CRITICAL: 3 } as const;

// High-risk command patterns for SSH auditing
const HIGH_RISK_PATTERNS = [
  // Destructive file operations
  { pattern: /rm\s+.*-.*rf|\s+-rf\s+.*rm/, risk: 'HIGH', description: 'Recursive force delete (rm -rf)' },
  { pattern: /dd\s+.*of=.*\/dev\/|\s+if=.*\/dev\//, risk: 'CRITICAL', description: 'Direct disk write (dd)' },
  { pattern: /mkfs\.|\s+mkfs\s+/, risk: 'HIGH', description: 'Filesystem creation (mkfs)' },
  { pattern: /fdisk\s+.*\/dev\/|parted\s+.*\/dev\//, risk: 'HIGH', description: 'Partition manipulation' },
  
  // Permission changes
  { pattern: /chmod\s+.*777|\s+777\s+.*chmod/, risk: 'MEDIUM', description: 'Dangerous permission change (chmod 777)' },
  { pattern: /chown\s+.*root:root|\s+root:root\s+.*chown/, risk: 'MEDIUM', description: 'Ownership change to root' },
  
  // Network and system manipulation
  { pattern: /iptables\s+.*--flush|\s+-F\s+.*iptables/, risk: 'HIGH', description: 'Firewall flush' },
  { pattern: /systemctl\s+.*stop\s+.*|service\s+.*stop\s+.*/, risk: 'MEDIUM', description: 'Service stop' },
  { pattern: /shutdown\s+.*now|halt\s+|poweroff/, risk: 'HIGH', description: 'System shutdown' },
  
  // Sensitive data access
  { pattern: /cat\s+.*\/etc\/shadow|\s+\/etc\/shadow\s+.*cat/, risk: 'HIGH', description: 'Shadow file access' },
  { pattern: /cat\s+.*\/etc\/passwd|\s+\/etc\/passwd\s+.*cat/, risk: 'MEDIUM', description: 'Passwd file access' },
  
  // Shell manipulation
  { pattern: />\s*\/dev\/|\s*\/dev\/.*>/, risk: 'MEDIUM', description: 'Output redirection to device' },
  { pattern: /wget\s+.*-O\s+.*\/|\s+-O\s+.*\/.*wget/, risk: 'MEDIUM', description: 'Download to system location' },
  { pattern: /curl\s+.*-o\s+.*\/|\s+-o\s+.*\/.*curl/, risk: 'MEDIUM', description: 'Curl to system location' },
];

export interface AuditEvent {
  id: string;
  timestamp: Date;
  sessionId?: string;
  userId?: string;
  command: string;
  riskLevel: 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL';
  description: string;
  detectedPatterns: string[];
  action: 'ALLOWED' | 'BLOCKED' | 'WARNED';
  details?: any;
}

// Backend AuditEvent (as defined in Rust)
interface BackendAuditEvent {
  id: string;
  timestamp: number; // u64 (milliseconds since epoch)
  session_id?: string;
  user_id?: string;
  command: string;
  risk_level: string;
  description: string;
  detected_patterns: string;
  action: string;
  details?: string;
}

export function toBackendAuditEvent(event: AuditEvent): BackendAuditEvent {
  return {
    id: event.id,
    timestamp: event.timestamp.getTime(),
    session_id: event.sessionId,
    user_id: event.userId,
    command: event.command,
    risk_level: event.riskLevel,
    description: event.description,
    detected_patterns: JSON.stringify(event.detectedPatterns),
    action: event.action,
    details: event.details ? JSON.stringify(event.details) : undefined,
  };
}

export class AuditService {
  private events: AuditEvent[] = [];
  private warningThreshold: 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL' = 'MEDIUM';
  
  analyzeCommand(command: string): Pick<AuditEvent, 'riskLevel' | 'detectedPatterns' | 'description'> {
    const detectedPatterns: string[] = [];
    let highestRisk: 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL' = 'LOW';
    const descriptions: string[] = [];
    
    for (const { pattern, risk, description } of HIGH_RISK_PATTERNS) {
      if (pattern.test(command)) {
        detectedPatterns.push(description);
        descriptions.push(description);
        
        // Update highest risk
        type RiskKey = keyof typeof RISK_ORDER;
        if (RISK_ORDER[risk as RiskKey] > RISK_ORDER[highestRisk]) {
          highestRisk = risk as 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL';
        }
      }
    }
    
    return {
      riskLevel: highestRisk,
      description: descriptions.length > 0 ? descriptions.join(', ') : 'No high-risk patterns detected',
      detectedPatterns
    };
  }

  shouldPrompt(riskLevel: 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL'): boolean {
    return RISK_ORDER[riskLevel] >= RISK_ORDER[this.warningThreshold];
  }

  requiresConfirmation(riskLevel: 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL'): boolean {
    return RISK_ORDER[riskLevel] >= RISK_ORDER.HIGH;
  }

  createEvent(params: {
    command: string;
    sessionId?: string;
    userId?: string;
    action: AuditEvent['action'];
    analysis?: Pick<AuditEvent, 'riskLevel' | 'detectedPatterns' | 'description'>;
    details?: any;
  }): AuditEvent {
    const analysis = params.analysis ?? this.analyzeCommand(params.command);
    return {
      id: crypto.randomUUID(),
      timestamp: new Date(),
      sessionId: params.sessionId,
      userId: params.userId,
      command: params.command,
      riskLevel: analysis.riskLevel,
      description: analysis.description,
      detectedPatterns: analysis.detectedPatterns,
      action: params.action,
      details: params.details
    };
  }

  cacheEvent(event: AuditEvent): void {
    this.events.push(event);
  }
  
  async recordEvent(event: AuditEvent): Promise<void> {
    // Send to backend logging system
    try {
      await invoke('log_audit_event', { event: toBackendAuditEvent(event) });
    } catch (error) {
      console.error('Failed to send audit event to backend:', error);
    }

    this.cacheEvent(event);
  }
  
  /**
   * Get all audit events (from memory cache)
   */
  getEvents(): AuditEvent[] {
    return [...this.events];
  }
  
  /**
   * Get events filtered by risk level
   */
  getEventsByRisk(risk: 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL'): AuditEvent[] {
    return this.events.filter(e => e.riskLevel === risk);
  }
  
  /**
   * Load audit events from backend database
   */
  async loadEventsFromBackend(): Promise<AuditEvent[]> {
    try {
      console.info('Backend audit reads are disabled; returning cached events only.');
      return [];
    } catch (error) {
      console.error('Failed to load audit events from backend:', error);
      return [];
    }
  }
  
  /**
   * Refresh events from backend and replace memory cache
   */
  async refreshEvents(): Promise<void> {
    try {
      console.info('Backend audit refresh is disabled; keeping local cache.');
    } catch (error) {
      console.error('Failed to refresh audit events:', error);
    }
  }
  
  /**
   * Clear audit history (memory only, backend data remains)
   */
  clearHistory(): void {
    this.events = [];
  }
  
  /**
   * Clear all audit events from backend database
   */
  async clearBackendHistory(): Promise<void> {
    try {
      console.info('Clearing backend audit history is disabled.');
    } catch (error) {
      console.error('Failed to clear backend audit events:', error);
    }
  }
  
  /**
   * Set warning threshold
   */
  setWarningThreshold(threshold: 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL'): void {
    this.warningThreshold = threshold;
  }
  
  /**
   * Check if command should be blocked based on threshold
   */
  shouldBlock(command: string): boolean {
    const event = this.analyzeCommand(command);
    return RISK_ORDER[event.riskLevel] >= RISK_ORDER[this.warningThreshold];
  }
}

export const auditService = new AuditService();
