export type ShortcutModifier = 'Ctrl' | 'Shift' | 'Alt' | 'Meta';

export type ParsedShortcut = {
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  meta: boolean;
  key: string;
};

const MODIFIER_ORDER: ShortcutModifier[] = ['Ctrl', 'Shift', 'Alt', 'Meta'];

const KEY_ALIASES: Record<string, string> = {
  esc: 'Escape',
  escape: 'Escape',
  return: 'Enter',
  enter: 'Enter',
  del: 'Delete',
  delete: 'Delete',
  backspace: 'Backspace',
  tab: 'Tab',
  space: 'Space',
  spacebar: 'Space',
  up: 'ArrowUp',
  down: 'ArrowDown',
  left: 'ArrowLeft',
  right: 'ArrowRight',
  pageup: 'PageUp',
  pagedown: 'PageDown',
  home: 'Home',
  end: 'End',
  insert: 'Insert',
  plus: 'Plus',
};

const CODE_KEY_ALIASES: Record<string, string> = {
  BracketLeft: '[',
  BracketRight: ']',
  Minus: '-',
  Equal: '=',
  Backquote: '`',
  Backslash: '\\',
  Slash: '/',
  Semicolon: ';',
  Quote: "'",
  Comma: ',',
  Period: '.',
  Space: 'Space',
  Enter: 'Enter',
  Tab: 'Tab',
  Delete: 'Delete',
  Backspace: 'Backspace',
  NumpadAdd: 'Plus',
  NumpadSubtract: '-',
  NumpadMultiply: '*',
  NumpadDivide: '/',
  NumpadDecimal: '.',
};

function isFunctionKey(value: string): boolean {
  return /^F\d{1,2}$/i.test(value);
}

function isModifierOnlyKey(value: string): boolean {
  const lower = value.toLowerCase();
  return (
    lower === 'control' ||
    lower === 'ctrl' ||
    lower === 'shift' ||
    lower === 'alt' ||
    lower === 'meta' ||
    lower === 'cmd' ||
    lower === 'command'
  );
}

export function normalizeKeyToken(keyRaw: string): string {
  const trimmed = keyRaw.trim();
  if (!trimmed) return '';

  const lower = trimmed.toLowerCase();
  if (KEY_ALIASES[lower]) return KEY_ALIASES[lower];
  if (isFunctionKey(trimmed)) return trimmed.toUpperCase();
  if (trimmed === '+') return 'Plus';
  if (trimmed === ' ') return 'Space';
  if (trimmed.length === 1) return trimmed.toUpperCase();

  return trimmed;
}

export function formatShortcut(modifiers: ShortcutModifier[], keyRaw: string): string {
  const key = normalizeKeyToken(keyRaw);
  if (!key) return '';

  const unique = Array.from(new Set(modifiers));
  const ordered = MODIFIER_ORDER.filter(modifier => unique.includes(modifier));
  return ordered.length ? `${ordered.join('+')}+${key}` : key;
}

export function normalizeShortcut(raw: string): { value: string } | { error: string } {
  const trimmed = raw.trim();
  if (!trimmed) return { value: '' };

  const parts = trimmed
    .split('+')
    .map(part => part.trim())
    .filter(Boolean);

  if (parts.length === 0) return { value: '' };

  const key = parts[parts.length - 1];
  const modifierParts = parts.slice(0, -1);

  const modifiers = new Set<ShortcutModifier>();
  for (const modifier of modifierParts) {
    const lower = modifier.toLowerCase();
    if (lower === 'ctrl' || lower === 'control') modifiers.add('Ctrl');
    else if (lower === 'shift') modifiers.add('Shift');
    else if (lower === 'alt' || lower === 'option') modifiers.add('Alt');
    else if (lower === 'meta' || lower === 'cmd' || lower === 'command') modifiers.add('Meta');
    else return { error: '格式错误：只允许修饰键 + 按键' };
  }

  const normalizedKey = normalizeKeyToken(key);
  if (!normalizedKey) return { value: '' };
  if (isModifierOnlyKey(normalizedKey)) return { error: '格式错误：必须包含一个非修饰键' };

  return { value: formatShortcut(Array.from(modifiers), normalizedKey) };
}

export function parseShortcut(shortcut: string): ParsedShortcut | null {
  const normalized = normalizeShortcut(shortcut);
  if ('error' in normalized || !normalized.value) return null;

  const parts = normalized.value.split('+');
  const key = parts.pop();
  if (!key) return null;

  return {
    ctrl: parts.includes('Ctrl'),
    shift: parts.includes('Shift'),
    alt: parts.includes('Alt'),
    meta: parts.includes('Meta'),
    key,
  };
}

export function getShortcutFromKeyboardEvent(event: KeyboardEvent): string | null {
  const key = getEventKey(event);
  if (!key || isModifierOnlyKey(key)) return null;

  const modifiers: ShortcutModifier[] = [];
  if (event.ctrlKey) modifiers.push('Ctrl');
  if (event.shiftKey) modifiers.push('Shift');
  if (event.altKey) modifiers.push('Alt');
  if (event.metaKey) modifiers.push('Meta');

  return formatShortcut(modifiers, key);
}

export function matchShortcut(event: KeyboardEvent, shortcut: string): boolean {
  const parsed = parseShortcut(shortcut);
  if (!parsed) return false;

  if (parsed.ctrl !== event.ctrlKey) return false;
  if (parsed.shift !== event.shiftKey) return false;
  if (parsed.alt !== event.altKey) return false;
  if (parsed.meta !== event.metaKey) return false;

  const eventKey = getEventKey(event);
  if (eventKey === parsed.key) return true;

  const codeKey = CODE_KEY_ALIASES[event.code];
  return codeKey === parsed.key;
}

export function isEditableShortcutTarget(
  target: EventTarget | null,
  options: { allowTerminalTextarea?: boolean } = {}
): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (
    options.allowTerminalTextarea &&
    Array.from(target.classList).some((name) => name.startsWith('xterm-helper-textarea'))
  ) {
    return false;
  }

  return (
    target.tagName === 'INPUT' ||
    target.tagName === 'TEXTAREA' ||
    target.isContentEditable
  );
}

function getEventKey(event: KeyboardEvent): string | null {
  const key = event.key?.trim();
  if (key) {
    const normalized = normalizeKeyToken(key);
    if (normalized) return normalized;
  }

  return CODE_KEY_ALIASES[event.code] ?? null;
}
