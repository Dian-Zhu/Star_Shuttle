import { invoke } from '@tauri-apps/api/core';
import { auditService, toBackendAuditEvent } from './auditService';
import { summarizeAuditedCommand } from './commandAuditSanitizer';

function summarizeRemoteCommand(command: string, riskLevel: string): string {
  return summarizeAuditedCommand(command, riskLevel, { maxLen: 200 });
}

export async function execAuditedRemoteCommand(
  sessionId: string,
  command: string,
  source: string,
): Promise<string> {
  const analysis = auditService.analyzeCommand(command);
  const auditCommand = summarizeRemoteCommand(command, analysis.riskLevel);
  const needsConfirmation = auditService.requiresConfirmation(analysis.riskLevel);
  const confirmed = !needsConfirmation
    || window.confirm(
      [
        `检测到高风险远程命令: ${auditCommand}`,
        `风险等级: ${analysis.riskLevel}`,
        `原因: ${analysis.description}`,
        '',
        '确认继续执行吗？',
      ].join('\n'),
    );

  const action = confirmed
    ? (needsConfirmation ? 'WARNED' : 'ALLOWED')
    : 'BLOCKED';
  const event = auditService.createEvent({
    command: auditCommand,
    sessionId,
    action,
    analysis,
    details: {
      source,
      riskLevel: analysis.riskLevel,
      detectedPatterns: analysis.detectedPatterns,
      commandWasSanitized: true,
    },
  });

  const result = await invoke<unknown>('exec_audited_command', {
    sessionId,
    command,
    auditEvent: toBackendAuditEvent(event),
    execute: confirmed,
  });
  auditService.cacheEvent(event);

  if (!confirmed) {
    throw new Error(`已阻止高风险命令: ${auditCommand}`);
  }

  return typeof result === 'string' ? result : String(result ?? '');
}
