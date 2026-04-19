const OSC7_PREFIX = '\u001b]7;file://';
const OSC_BEL = '\u0007';
const OSC_ST = '\u001b\\';
const MAX_REMAINDER_LENGTH = 4096;

function findOscTerminator(input: string, start: number): number {
  const belIndex = input.indexOf(OSC_BEL, start);
  const stIndex = input.indexOf(OSC_ST, start);
  if (belIndex === -1) return stIndex;
  if (stIndex === -1) return belIndex;
  return Math.min(belIndex, stIndex);
}

function decodeOsc7Path(payload: string): string | null {
  if (!payload.startsWith('file://')) return null;

  try {
    const url = new URL(payload);
    const path = decodeURIComponent(url.pathname || '/');
    return path || '/';
  } catch {
    const withoutScheme = payload.slice('file://'.length);
    const slashIndex = withoutScheme.indexOf('/');
    if (slashIndex < 0) return null;
    const rawPath = withoutScheme.slice(slashIndex);
    if (!rawPath) return null;
    try {
      return decodeURIComponent(rawPath);
    } catch {
      return rawPath;
    }
  }
}

export function extractTerminalWorkingDirectory(chunk: string, previousRemainder = ''): {
  cwd: string | null;
  remainder: string;
} {
  const input = `${previousRemainder}${chunk}`;
  let searchIndex = 0;
  let cwd: string | null = null;
  let trailingStart = -1;

  for (;;) {
    const start = input.indexOf(OSC7_PREFIX, searchIndex);
    if (start === -1) break;

    const payloadStart = start + '\u001b]7;'.length;
    const terminator = findOscTerminator(input, payloadStart);
    if (terminator === -1) {
      trailingStart = start;
      break;
    }

    const payload = input.slice(payloadStart, terminator);
    const decoded = decodeOsc7Path(payload);
    if (decoded) {
      cwd = decoded;
    }

    searchIndex = terminator + (input.startsWith(OSC_ST, terminator) ? OSC_ST.length : OSC_BEL.length);
  }

  let remainder = trailingStart >= 0 ? input.slice(trailingStart) : '';
  if (remainder.length > MAX_REMAINDER_LENGTH) {
    remainder = remainder.slice(-MAX_REMAINDER_LENGTH);
  }

  return { cwd, remainder };
}

