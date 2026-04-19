import { describe, expect, it, vi } from 'vitest'
import {
  attachAlternateBufferPreserver,
  collectBufferText,
  formatPreservedAltBufferOutput,
  type BufferLike,
  type BufferLineLike,
  type TerminalLike,
} from './terminalAltBuffer'

function createBufferLine(text: string, isWrapped = false): BufferLineLike {
  return {
    isWrapped,
    translateToString: () => text,
  }
}

function createBuffer(type: 'normal' | 'alternate', lines: BufferLineLike[]): BufferLike {
  return {
    type,
    length: lines.length,
    getLine: (index: number) => lines[index],
  }
}

function emitBufferChange(
  listener: ((buffer: BufferLike) => void) | null,
  buffer: BufferLike
): void {
  if (!listener) {
    throw new Error('Expected buffer change listener to be registered')
  }
  listener(buffer)
}

describe('terminalAltBuffer', () => {
  it('merges wrapped buffer lines and trims empty edges', () => {
    const buffer = createBuffer('alternate', [
      createBufferLine(''),
      createBufferLine('Hello'),
      createBufferLine(' world', true),
      createBufferLine(''),
      createBufferLine('Done'),
      createBufferLine(''),
    ])

    expect(collectBufferText(buffer)).toBe('Hello world\n\nDone')
  })

  it('formats preserved alternate buffer output for terminal write', () => {
    expect(formatPreservedAltBufferOutput('line1\nline2')).toContain(
      '[Preserved full-screen output]'
    )
    expect(formatPreservedAltBufferOutput('line1\nline2')).toContain('line1\r\nline2\r\n')
  })

  it('writes alternate buffer snapshot when switching back to normal buffer', () => {
    const alternate = createBuffer('alternate', [
      createBufferLine('Codex login'),
      createBufferLine('Cancelled'),
    ])
    const normal = createBuffer('normal', [createBufferLine('$ ')])

    let active = alternate
    let listener: ((buffer: BufferLike) => void) | null = null
    const writes: string[] = []

    const terminal: TerminalLike = {
      buffer: {
        get active() {
          return active
        },
        get alternate() {
          return alternate
        },
        onBufferChange(callback) {
          listener = callback
          return {
            dispose() {
              listener = null
            },
          }
        },
      },
      write(data: string) {
        writes.push(data)
      },
    }

    attachAlternateBufferPreserver(terminal, {
      schedule: (callback) => callback(),
    })

    active = normal
    emitBufferChange(listener, normal)

    expect(writes).toHaveLength(1)
    expect(writes[0]).toContain('Codex login\r\nCancelled')
  })

  it('does not write duplicate snapshots repeatedly', () => {
    const alternate = createBuffer('alternate', [createBufferLine('same screen')])
    const normal = createBuffer('normal', [createBufferLine('$ ')])

    let active = alternate
    let listener: ((buffer: BufferLike) => void) | null = null
    const write = vi.fn()

    const terminal: TerminalLike = {
      buffer: {
        get active() {
          return active
        },
        get alternate() {
          return alternate
        },
        onBufferChange(callback) {
          listener = callback
          return {
            dispose() {
              listener = null
            },
          }
        },
      },
      write,
    }

    attachAlternateBufferPreserver(terminal, {
      schedule: (callback) => callback(),
    })

    active = normal
    emitBufferChange(listener, normal)
    active = alternate
    emitBufferChange(listener, alternate)
    active = normal
    emitBufferChange(listener, normal)

    expect(write).toHaveBeenCalledTimes(1)
  })
})
