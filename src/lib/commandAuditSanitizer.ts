export type AuditRiskLevel = 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL' | string;

const VARIABLE_SECRET_PATTERN =
  /\b([A-Za-z_][A-Za-z0-9_]*(?:pass(word)?|token|secret|api[_-]?key|access[_-]?key|private[_-]?key|pwd))\s*=\s*(?:'[^']*'|"[^"]*"|[^\s;]+)/gi;
const LONG_OPTION_SECRET_PATTERN =
  /(--?(?:password|passphrase|passwd|token|secret|api[_-]?key|access[_-]?key|private[_-]?key)\b(?:=|\s+))(?:'[^']*'|"[^"]*"|[^\s;]+)/gi;
const SSHPASS_COMPACT_SECRET_PATTERN =
  /\b(sshpass\s+-p)(?:'[^']*'|"[^"]*"|[^\s;]+)/gi;
const SHORT_PASSWORD_SECRET_PATTERN =
  /(^|\s)(-p)\s+(?:'[^']*'|"[^"]*"|[^\s;]+)/g;
const URL_SECRET_PATTERN = /(https?:\/\/[^/\s:@]+:)[^@\s]+@/gi;
const JWT_PATTERN = /\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b/g;
const BEARER_PATTERN = /\b(Bearer)\s+(?:'[^']*'|"[^"]*"|[^\s;]+)/gi;

type SummarizeOptions = {
  maxLen?: number;
};

export function summarizeAuditedCommand(
  command: string,
  riskLevel: AuditRiskLevel,
  options: SummarizeOptions = {}
): string {
  const maxLen = options.maxLen ?? 200;
  let summary = command.replace(/\s+/g, ' ').trim();
  if (!summary) return '<empty>';

  summary = summary.replace(VARIABLE_SECRET_PATTERN, '$1=<redacted>');
  summary = summary.replace(LONG_OPTION_SECRET_PATTERN, '$1<redacted>');
  summary = summary.replace(SSHPASS_COMPACT_SECRET_PATTERN, '$1<redacted>');
  summary = summary.replace(SHORT_PASSWORD_SECRET_PATTERN, '$1$2 <redacted>');
  summary = summary.replace(URL_SECRET_PATTERN, '$1<redacted>@');
  summary = summary.replace(JWT_PATTERN, '<redacted-jwt>');
  summary = summary.replace(BEARER_PATTERN, '$1 <redacted>');

  if (riskLevel === 'HIGH' || riskLevel === 'CRITICAL') {
    const head = summary.split(/\s+/).slice(0, 4).join(' ');
    summary = `[${riskLevel}] ${head || '<command>'} [summary]`;
  }

  if (summary.length > maxLen) {
    summary = `${summary.slice(0, maxLen)}...`;
  }

  return summary;
}
