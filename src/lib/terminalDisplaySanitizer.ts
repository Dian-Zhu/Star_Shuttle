const CONTROL_SEQUENCE_PATTERN = /\x1b(?:\][^\x07\x1b]*(?:\x07|\x1b\\)|\[[0-?]*[ -/]*[@-~]|[@-_])/g;
const OTHER_CONTROL_PATTERN = /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/g;

export function sanitizeTerminalDisplayText(value: unknown): string {
  const text = typeof value === 'string' ? value : String(value ?? '');
  return text
    .replace(CONTROL_SEQUENCE_PATTERN, '')
    .replace(OTHER_CONTROL_PATTERN, '')
    .replace(/\r/g, '');
}
