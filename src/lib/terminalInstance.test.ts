import { beforeEach, describe, expect, it, vi } from 'vitest';

type MockElement = {
  className: string;
  parentElement: MockContainer | null;
  addEventListener: (event: string, handler: EventListenerOrEventListenerObject) => void;
  removeEventListener: (event: string, handler: EventListenerOrEventListenerObject) => void;
};

type MockContainer = {
  children: MockElement[];
  appendChild: (child: MockElement) => MockElement;
  removeChild: (child: MockElement) => MockElement;
  querySelector: (selector: string) => MockElement | null;
};

function createMockContainer(): MockContainer {
  return {
    children: [],
    appendChild(child) {
      if (child.parentElement && child.parentElement !== this) {
        child.parentElement.removeChild(child);
      }
      if (!this.children.includes(child)) {
        this.children.push(child);
      }
      child.parentElement = this;
      return child;
    },
    removeChild(child) {
      this.children = this.children.filter((item) => item !== child);
      child.parentElement = null;
      return child;
    },
    querySelector(selector) {
      if (selector !== '.xterm') return null;
      return this.children.find((child) => child.className.includes('xterm')) ?? null;
    }
  };
}

const containerManagerMock = vi.hoisted(() => ({
  ensureClean: vi.fn(),
  cleanupContainer: vi.fn((container: MockContainer) => {
    for (const child of [...container.children]) {
      container.removeChild(child);
    }
  })
}));

const focusMock = vi.hoisted(() => vi.fn());
const openMock = vi.hoisted(() => vi.fn());

vi.mock('./containerManager', () => ({
  containerManager: containerManagerMock
}));

vi.mock('./devLogger', () => ({
  devLog: vi.fn(),
  devWarn: vi.fn()
}));

vi.mock('@xterm/xterm', () => ({
  Terminal: class MockTerminal {
    element: MockElement | null = null;
    textarea = null;
    options = {};
    open(container: MockContainer) {
      openMock(container);
      if (!this.element) {
        this.element = {
          className: 'xterm',
          parentElement: null,
          addEventListener() {},
          removeEventListener() {}
        };
      }
      container.appendChild(this.element);
    }
    focus() {
      focusMock();
    }
    loadAddon() {}
    onData() {
      return { dispose() {} };
    }
    onKey() {
      return { dispose() {} };
    }
    onLineFeed() {
      return { dispose() {} };
    }
    onScroll() {
      return { dispose() {} };
    }
    onSelectionChange() {
      return { dispose() {} };
    }
    onResize() {
      return { dispose() {} };
    }
    write() {}
    writeln() {}
    clear() {}
    dispose() {}
    getSelection() {
      return '';
    }
    reset() {}
  }
}));

vi.mock('@xterm/addon-fit', () => ({
  FitAddon: class MockFitAddon {
    fit = vi.fn();
  }
}));

vi.mock('@xterm/addon-search', () => ({
  SearchAddon: class MockSearchAddon {}
}));

vi.mock('@xterm/addon-web-links', () => ({
  WebLinksAddon: class MockWebLinksAddon {}
}));

describe('TerminalInstance remount behavior', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  it('reuses the existing xterm DOM when remounted into a new container', async () => {
    const { Terminal } = await import('@xterm/xterm');
    const { FitAddon } = await import('@xterm/addon-fit');
    const { SearchAddon } = await import('@xterm/addon-search');
    const { TerminalInstance } = await import('./terminalInstance');

    const terminal = new Terminal() as any;
    const fitAddon = new FitAddon() as any;
    const searchAddon = new SearchAddon() as any;
    const instance = TerminalInstance.fromInitialized('session-1', terminal, fitAddon, searchAddon);

    const firstContainer = createMockContainer();
    const secondContainer = createMockContainer();

    instance.mount(firstContainer as any);
    const originalElement = terminal.element;

    expect(openMock).toHaveBeenCalledTimes(1);
    expect(firstContainer.children).toHaveLength(1);
    expect(originalElement?.parentElement).toBe(firstContainer);

    instance.unmount();

    expect(firstContainer.children).toHaveLength(0);
    expect(originalElement?.parentElement).toBeNull();

    instance.mount(secondContainer as any);

    expect(openMock).toHaveBeenCalledTimes(1);
    expect(secondContainer.children).toHaveLength(1);
    expect(secondContainer.children[0]).toBe(originalElement);
    expect(originalElement?.parentElement).toBe(secondContainer);
    expect(focusMock).toHaveBeenCalledTimes(2);
  });
});
