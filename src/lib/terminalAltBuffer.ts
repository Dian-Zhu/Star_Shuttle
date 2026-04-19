export type TerminalBufferType = 'normal' | 'alternate'

export type BufferLineLike = {
  isWrapped: boolean
  translateToString: (trimRight?: boolean, startColumn?: number, endColumn?: number) => string
}

export type BufferLike = {
  type: TerminalBufferType
  length: number
  getLine: (index: number) => BufferLineLike | undefined
}

export type BufferNamespaceLike = {
  active: BufferLike
  alternate: BufferLike
  onBufferChange: (listener: (buffer: BufferLike) => void) => { dispose: () => void }
}

export type TerminalLike = {
  buffer: BufferNamespaceLike
  write: (data: string) => void
}

type AltBufferPreserverOptions = {
  schedule?: (callback: () => void) => void
  write?: (data: string) => void
}

const PRESERVED_ALT_BUFFER_HEADER = '\r\n\x1b[90m[Preserved full-screen output]\x1b[0m\r\n'

function trimEdgeEmptyLines(lines: string[]): string[] {
  let start = 0
  let end = lines.length

  while (start < end && lines[start].trim().length === 0) {
    start++
  }

  while (end > start && lines[end - 1].trim().length === 0) {
    end--
  }

  return lines.slice(start, end)
}

export function collectBufferText(buffer: BufferLike): string {
  const mergedLines: string[] = []
  let currentLine: string | null = null

  for (let index = 0; index < buffer.length; index++) {
    const line = buffer.getLine(index)
    if (!line) continue

    const text = line.translateToString(true)
    if (line.isWrapped) {
      currentLine = `${currentLine ?? ''}${text}`
      continue
    }

    if (currentLine !== null) {
      mergedLines.push(currentLine)
    }
    currentLine = text
  }

  if (currentLine !== null) {
    mergedLines.push(currentLine)
  }

  return trimEdgeEmptyLines(mergedLines).join('\n')
}

export function formatPreservedAltBufferOutput(text: string): string {
  const normalized = text.replace(/\r\n/g, '\n').replace(/\r/g, '\n')
  return `${PRESERVED_ALT_BUFFER_HEADER}${normalized.replace(/\n/g, '\r\n')}\r\n`
}

function defaultSchedule(callback: () => void) {
  if (typeof queueMicrotask === 'function') {
    queueMicrotask(callback)
    return
  }
  setTimeout(callback, 0)
}

export function attachAlternateBufferPreserver(
  term: TerminalLike,
  options: AltBufferPreserverOptions = {}
) {
  let lastActiveBufferType = term.buffer.active.type
  let lastPreservedSnapshot = ''

  return term.buffer.onBufferChange((nextBuffer) => {
    const previousBufferType = lastActiveBufferType
    lastActiveBufferType = nextBuffer.type

    if (previousBufferType !== 'alternate' || nextBuffer.type !== 'normal') {
      return
    }

    const snapshot = collectBufferText(term.buffer.alternate)
    if (!snapshot.trim() || snapshot === lastPreservedSnapshot) {
      return
    }

    lastPreservedSnapshot = snapshot
    const schedule = options.schedule ?? defaultSchedule
    const write = options.write ?? ((data: string) => term.write(data))

    schedule(() => {
      write(formatPreservedAltBufferOutput(snapshot))
    })
  })
}
