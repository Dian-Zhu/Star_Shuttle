import { invoke } from '@tauri-apps/api/core';
import { auditService } from './auditService';

function summarizeRemoteCommand(command: string, riskLevel: string): string {
  let summary = command.replace(/\s+/g, ' ').trim();
  if (!summary) return '<empty>';

  summary = summary.replace(
    /\b([A-Za-z_][A-Za-z0-9_]*(?:pass(word)?|token|secret|api[_-]?key|access[_-]?key|private[_-]?key|pwd))\s*=\s*(?:'[^']*'|"[^"]*"|[^\s;]+)/gi,
    '$1=<redacted>',
  );
  summary = summary.replace(
    /(--?(?:password|passphrase|passwd|token|secret|api[_-]?key|access[_-]?key|private[_-]?key)\b(?:=|\s+))(?:'[^']*'|"[^"]*"|[^\s;]+)/gi,
    '$1<redacted>',
  );
  summary = summary.replace(
    /(https?:\/\/[^/\s:@]+:)[^@\s]+@/gi,
    '$1<redacted>@',
  );

  if (riskLevel === 'HIGH' || riskLevel === 'CRITICAL') {
    const head = summary.split(/\s+/).slice(0, 4).join(' ');
    summary = `[${riskLevel}] ${head || '<command>'} [summary]`;
  }

  if (summary.length > 200) {
    summary = `${summary.slice(0, 200)}...`;
  }

  return summary;
}

export async function execAuditedRemoteCommand(
  sessionId: string,
  command: string,
  source: string,
): Promise<string> {
  const analysis = auditService.analyzeCommand(command);
  const auditCommand = summarizeRemoteCommand(command, analysis.riskLevel);

  if (auditService.requiresConfirmation(analysis.riskLevel)) {
    const confirmed = window.confirm(
      [
        `检测到高风险远程命令: ${auditCommand}`,
        `风险等级: ${analysis.riskLevel}`,
        `原因: ${analysis.description}`,
        '',
        '确认继续执行吗？',
      ].join('\n'),
    );

    await auditService.recordEvent(
      auditService.createEvent({
        command: auditCommand,
        sessionId,
        action: confirmed ? 'WARNED' : 'BLOCKED',
        details: {
          source,
          riskLevel: analysis.riskLevel,
          detectedPatterns: analysis.detectedPatterns,
          commandWasSanitized: true,
        },
      }),
    );

    if (!confirmed) {
      throw new Error(`已阻止高风险命令: ${auditCommand}`);
    }
  } else {
    await auditService.recordEvent(
      auditService.createEvent({
        command: auditCommand,
        sessionId,
        action: 'ALLOWED',
        details: {
          source,
          riskLevel: analysis.riskLevel,
          detectedPatterns: analysis.detectedPatterns,
          commandWasSanitized: true,
        },
      }),
    );
  }

  const result = await invoke('exec_command', { sessionId, command });
  return typeof result === 'string' ? result : String(result ?? '');
}
