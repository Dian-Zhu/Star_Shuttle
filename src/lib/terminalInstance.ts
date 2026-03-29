/**
 * 独立终端实例
 * 
 * 封装完整的终端逻辑，不依赖组件生命周期
 * 可以在任何时候挂载到不同的容器
 */

import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { SearchAddon } from '@xterm/addon-search';
import { WebLinksAddon } from '@xterm/addon-web-links';
import type { ITerminalOptions } from '@xterm/xterm';
import { containerManager } from './containerManager';
import { devLog, devWarn } from './devLogger';

type TerminalEventHandler = (...args: unknown[]) => void;
type TerminalDisposable = { dispose: () => void };
type DisposableEventFactory<THandler> = (handler: THandler) => TerminalDisposable;
function getDisposableEventFactory<THandler>(
  terminal: object,
  eventName: 'onBinary' | 'onBell' | 'onTitleChange'
): DisposableEventFactory<THandler> | null {
  const candidate = (terminal as Record<string, unknown>)[eventName];
  if (typeof candidate !== 'function') {
    return null;
  }
  return candidate as DisposableEventFactory<THandler>;
}

export class TerminalInstance {
  private sessionIdValue: string;
  readonly terminal: Terminal;
  readonly fitAddon: FitAddon;
  readonly searchAddon: SearchAddon;
  readonly webLinksAddon: WebLinksAddon;

  private mountedContainer: HTMLElement | null = null;
  private isDisposed: boolean = false;

  // xterm.js Disposable 存储映射
  private disposables = new Map<string, Map<TerminalEventHandler, TerminalDisposable>>();

  // DOM 事件监听器（用于 focus 事件）
  private domEventListeners = new Map<string, TerminalEventHandler[]>();

  // Focus 事件监听器
  private focusListener: (() => void) | null = null;
  private blurListener: (() => void) | null = null;
  private hasFocusListener = false;
  private hasBlurListener = false;

  public constructor(
    sessionId: string,
    options?: ITerminalOptions,
    existingTerminal?: Terminal,
    existingFitAddon?: FitAddon,
    existingSearchAddon?: SearchAddon
  ) {
    this.sessionIdValue = sessionId;

    devLog(
      'TerminalInstance',
      `constructor session=${sessionId}, existingTerminal=${!!existingTerminal}, existingFitAddon=${!!existingFitAddon}, existingSearchAddon=${!!existingSearchAddon}`
    );

    // 如果提供了已经初始化好的组件，直接使用
    if (existingTerminal && existingFitAddon && existingSearchAddon) {
      this.terminal = existingTerminal;
      this.fitAddon = existingFitAddon;
      this.searchAddon = existingSearchAddon;
      this.webLinksAddon = null as any; // WebLinksAddon 不需要重新创建
      devLog('TerminalInstance', `using existing terminal session=${sessionId}, terminal=${!!this.terminal}`);
    } else {
      // 创建新的 Terminal 实例
      this.terminal = new Terminal({
        ...options,
        allowProposedApi: true,
        cursorBlink: true,
        fontSize: 14,
        fontFamily: 'Menlo, Monaco, "Courier New", monospace',
        theme: {
          background: '#1e1e1e',
          foreground: '#d4d4d4',
          cursor: '#ffffff',
          cursorAccent: '#1e1e1e',
          selectionBackground: '#264f78',
        },
      });
      devLog('TerminalInstance', `new terminal created session=${sessionId}, terminal=${!!this.terminal}`);

      // 创建并加载插件
      this.fitAddon = new FitAddon();
      this.searchAddon = new SearchAddon();
      this.webLinksAddon = new WebLinksAddon();

      this.terminal.loadAddon(this.fitAddon);
      this.terminal.loadAddon(this.searchAddon);
      this.terminal.loadAddon(this.webLinksAddon);
      devLog('TerminalInstance', `addons loaded session=${sessionId}`);
    }
  }

  /**
   * 从已经初始化好的 Terminal 对象创建实例
   * @param sessionId 会话 ID
   * @param terminal 已经初始化好的 Terminal 实例
   * @param fitAddon FitAddon 实例
   * @param searchAddon SearchAddon 实例
   * @returns TerminalInstance 实例
   */
  static fromInitialized(sessionId: string, terminal: Terminal, fitAddon: FitAddon, searchAddon: SearchAddon): TerminalInstance {
    return new TerminalInstance(
      sessionId,
      undefined,
      terminal,
      fitAddon,
      searchAddon
    );
  }

  get sessionId(): string {
    return this.sessionIdValue;
  }

  renameSession(nextSessionId: string): void {
    this.sessionIdValue = nextSessionId;
  }

  /**
   * 挂载到容器（幂等操作，可重复调用）
   * @param container 目标容器
   */
  mount(container: HTMLElement): void {
    if (this.isDisposed) {
      devWarn('TerminalInstance', `mount ignored for disposed session=${this.sessionId}`);
      return;
    }

    // 如果已经挂载到同一个容器，不做任何操作
    if (this.mountedContainer === container) {
      return;
    }

    // 如果已经挂载到其他容器，先卸载
    if (this.mountedContainer && this.mountedContainer !== container) {
      this.unmount();
    }

    // 确保容器干净
    containerManager.ensureClean(container);

    const terminalElement = this.terminal.element;

    if (!terminalElement) {
      // 首次挂载，直接 open
      this.terminal.open(container);
      this._ensureFocusListener();
      this._ensureBlurListener();
    } else {
      // 终端已经初始化过，直接复用现有 DOM，避免重复 open() 导致会话内容丢失
      if (terminalElement.parentElement !== container) {
        if (terminalElement.parentElement) {
          terminalElement.parentElement.removeChild(terminalElement);
        }
        container.appendChild(terminalElement);
      }
      this._ensureFocusListener();
      this._ensureBlurListener();
    }

    this.mountedContainer = container;

    // 调整大小
    this.fit();

    // 聚焦
    this.terminal.focus();
  }

  /**
   * 从容器卸载
   */
  unmount(): void {
    if (!this.mountedContainer) {
      return;
    }

    const terminalElement = this.terminal.element;
    if (terminalElement && terminalElement.parentElement === this.mountedContainer) {
      this.mountedContainer.removeChild(terminalElement);
    }
    containerManager.cleanupContainer(this.mountedContainer);

    this.mountedContainer = null;
  }

  /**
   * 调整终端大小
   */
  fit(): void {
    if (this.isDisposed || !this.mountedContainer) {
      return;
    }

    try {
      this.fitAddon.fit();
    } catch (error) {
      console.warn(`[TerminalInstance] Failed to fit terminal for session ${this.sessionId}:`, error);
    }
  }

  /**
   * 聚焦终端
   */
  focus(): void {
    if (this.isDisposed) {
      return;
    }

    this.terminal.focus();
  }

  /**
   * 写入数据到终端
   * @param data 要写入的数据
   */
  write(data: string): void {
    if (this.isDisposed) {
      return;
    }

    this.terminal.write(data);
  }

  /**
   * 写入换行符
   */
  writeln(data: string): void {
    if (this.isDisposed) {
      return;
    }

    this.terminal.writeln(data);
  }

  /**
   * 清空终端
   */
  clear(): void {
    if (this.isDisposed) {
      return;
    }

    this.terminal.clear();
  }

  /**
   * 订阅事件
   * @param event 事件名称
   * @param handler 事件处理函数
   */
  on(event: string, handler: TerminalEventHandler): void {
    if (this.isDisposed) {
      devWarn(
        'TerminalInstance',
        `subscribe ignored event=${event} on disposed session=${this.sessionId}`
      );
      return;
    }

    if (!this.terminal) {
      console.error(`[TerminalInstance] terminal is undefined for session ${this.sessionId}`);
      throw new Error(`Terminal is undefined for session ${this.sessionId}`);
    }

    // xterm.js 事件 API
    switch (event) {
      case 'data': {
        const dataDisposable = this.terminal.onData(handler as (data: string) => void);
        this._storeDisposable('data', handler, dataDisposable);
        break;
      }

      case 'key': {
        const keyDisposable = this.terminal.onKey(handler as (event: { key: string, domEvent: KeyboardEvent }) => void);
        this._storeDisposable('key', handler, keyDisposable);
        break;
      }

      case 'linefeed': {
        const linefeedDisposable = this.terminal.onLineFeed(handler as () => void);
        this._storeDisposable('linefeed', handler, linefeedDisposable);
        break;
      }

      case 'scroll': {
        const scrollDisposable = this.terminal.onScroll(handler as (newCursorPosition: number) => void);
        this._storeDisposable('scroll', handler, scrollDisposable);
        break;
      }

      case 'selection': {
        const selectionDisposable = this.terminal.onSelectionChange(handler as () => void);
        this._storeDisposable('selection', handler, selectionDisposable);
        break;
      }

      case 'resize': {
        const resizeDisposable = this.terminal.onResize(handler as (size: { cols: number, rows: number }) => void);
        this._storeDisposable('resize', handler, resizeDisposable);
        break;
      }

      case 'binary': {
        const onBinary = getDisposableEventFactory<(data: string) => void>(this.terminal, 'onBinary');
        if (!onBinary) {
          devWarn('TerminalInstance', `binary event unavailable for session=${this.sessionId}`);
          break;
        }
        const binaryDisposable = onBinary(handler as (data: string) => void);
        this._storeDisposable('binary', handler, binaryDisposable);
        break;
      }

      case 'bell': {
        const onBell = getDisposableEventFactory<() => void>(this.terminal, 'onBell');
        if (!onBell) {
          devWarn('TerminalInstance', `bell event unavailable for session=${this.sessionId}`);
          break;
        }
        const bellDisposable = onBell(handler as () => void);
        this._storeDisposable('bell', handler, bellDisposable);
        break;
      }

      case 'title': {
        const onTitleChange = getDisposableEventFactory<(title: string) => void>(
          this.terminal,
          'onTitleChange'
        );
        if (!onTitleChange) {
          devWarn('TerminalInstance', `title event unavailable for session=${this.sessionId}`);
          break;
        }
        const titleDisposable = onTitleChange(handler as (title: string) => void);
        this._storeDisposable('title', handler, titleDisposable);
        break;
      }

      case 'focus':
        // Focus 事件通过 DOM 事件监听实现
        this._addFocusHandler(handler);
        break;

      case 'blur':
        // Blur 事件通过 DOM 事件监听实现
        this._addBlurHandler(handler);
        break;

      default:
        devWarn(
          'TerminalInstance',
          `unsupported xterm event=${event} for session=${this.sessionId}`
        );
    }
  }

  /**
   * 添加焦点事件处理函数（通过DOM事件）
   */
  private _addFocusHandler(handler: TerminalEventHandler): void {
    if (!this.domEventListeners.has('focus')) {
      this.domEventListeners.set('focus', []);
    }
    this.domEventListeners.get('focus')!.push(handler);
    this._ensureFocusListener();
  }

  /**
   * 添加失焦事件处理函数（通过DOM事件）
   */
  private _addBlurHandler(handler: TerminalEventHandler): void {
    if (!this.domEventListeners.has('blur')) {
      this.domEventListeners.set('blur', []);
    }
    this.domEventListeners.get('blur')!.push(handler);
    this._ensureBlurListener();
  }

  /**
   * 确保焦点事件监听器已设置（通过DOM事件）
   */
  private _ensureFocusListener(): void {
    if (this.hasFocusListener || !this.terminal || !this.terminal.element) {
      return;
    }

    this.focusListener = () => {
      const focusHandlers = this.domEventListeners.get('focus');
      if (focusHandlers) {
        focusHandlers.forEach(handler => {
          try {
            handler();
          } catch (error) {
            console.error(`[TerminalInstance] Error in focus handler for session ${this.sessionId}:`, error);
          }
        });
      }
    };

    this.terminal.element.addEventListener('focus', this.focusListener);
    this.hasFocusListener = true;
    devLog('TerminalInstance', `focus listener attached session=${this.sessionId}`);
  }

  /**
   * 确保失焦事件监听器已设置（通过DOM事件）
   */
  private _ensureBlurListener(): void {
    if (this.hasBlurListener || !this.terminal || !this.terminal.element) {
      return;
    }

    this.blurListener = () => {
      const blurHandlers = this.domEventListeners.get('blur');
      if (blurHandlers) {
        blurHandlers.forEach(handler => {
          try {
            handler();
          } catch (error) {
            console.error(`[TerminalInstance] Error in blur handler for session ${this.sessionId}:`, error);
          }
        });
      }
    };

    // 保存 blur 监听器引用以便后续清理
    this.terminal.element.addEventListener('blur', this.blurListener);
    this.hasBlurListener = true;
    devLog('TerminalInstance', `blur listener attached session=${this.sessionId}`);
  }

  /**
   * 存储 xterm.js Disposable
   */
  private _storeDisposable(event: string, handler: TerminalEventHandler, disposable: TerminalDisposable): void {
    if (!this.disposables.has(event)) {
      this.disposables.set(event, new Map());
    }
    this.disposables.get(event)!.set(handler, disposable);
    devLog('TerminalInstance', `event subscribed session=${this.sessionId}, event=${event}`);
  }

  /**
   * 取消订阅事件
   * @param event 事件名称
   * @param handler 事件处理函数
   */
  off(event: string, handler: TerminalEventHandler): void {
    if (this.isDisposed) {
      return;
    }

    // xterm.js API 清理
    switch (event) {
      case 'data':
      case 'key':
      case 'linefeed':
      case 'scroll':
      case 'selection':
      case 'resize':
      case 'binary':
      case 'bell':
      case 'title': {
        const eventDisposables = this.disposables.get(event);
        if (eventDisposables) {
          const disposable = eventDisposables.get(handler);
          if (disposable && typeof disposable.dispose === 'function') {
            try {
              disposable.dispose();
              devLog('TerminalInstance', `event unsubscribed session=${this.sessionId}, event=${event}`);
            } catch (error) {
              console.warn(`[TerminalInstance] Failed to dispose event handler for ${event}:`, error);
            }
          }
          eventDisposables.delete(handler);
        }
        break;
      }

      case 'focus': {
        // Focus 事件处理
        const focusHandlers = this.domEventListeners.get('focus');
        if (focusHandlers) {
          const index = focusHandlers.indexOf(handler);
          if (index !== -1) {
            focusHandlers.splice(index, 1);
          }
          if (focusHandlers.length === 0) {
            // 没有更多的焦点处理器，移除DOM监听器
            this._removeFocusListener();
          }
        }
        break;
      }

      case 'blur': {
        // Blur 事件处理
        const blurHandlers = this.domEventListeners.get('blur');
        if (blurHandlers) {
          const index = blurHandlers.indexOf(handler);
          if (index !== -1) {
            blurHandlers.splice(index, 1);
          }
          if (blurHandlers.length === 0) {
            // 没有更多的失焦处理器，移除DOM监听器
            this._removeBlurListener();
          }
        }
        break;
      }
    }
  }

  /**
   * 清理所有事件处理函数
   */
  clearEventHandlers(): void {
    if (!this.terminal) {
      console.warn(`[TerminalInstance] Cannot clear event handlers: terminal is undefined for session ${this.sessionId}`);
      return;
    }

    // 清理所有 xterm.js disposables
    this.disposables.forEach((disposableMap, event) => {
      disposableMap.forEach(disposable => {
        try {
          if (disposable && typeof disposable.dispose === 'function') {
            disposable.dispose();
          }
        } catch (error) {
          console.warn(`[TerminalInstance] Failed to dispose event handler for ${event}:`, error);
        }
      });
    });
    this.disposables.clear();

    // 清理 DOM 事件监听器
    this._removeFocusListener();
    this._removeBlurListener();
    this.domEventListeners.clear();

    devLog('TerminalInstance', `all event handlers cleared session=${this.sessionId}`);
  }

  /**
   * 移除焦点事件监听器
   */
  private _removeFocusListener(): void {
    if (this.focusListener && this.terminal && this.terminal.element) {
      this.terminal.element.removeEventListener('focus', this.focusListener);
      this.focusListener = null;
      this.hasFocusListener = false;
      devLog('TerminalInstance', `focus listener removed session=${this.sessionId}`);
    }
  }

  /**
   * 移除失焦事件监听器
   */
  private _removeBlurListener(): void {
    if (this.blurListener && this.terminal && this.terminal.element) {
      this.terminal.element.removeEventListener('blur', this.blurListener);
      this.blurListener = null;
      this.hasBlurListener = false;
      devLog('TerminalInstance', `blur listener removed session=${this.sessionId}`);
    }
  }

  /**
   * 销毁实例
   */
  dispose(): void {
    if (this.isDisposed) {
      return;
    }

    // 清理所有事件处理函数（包括 xterm.js disposables 和 DOM 监听器）
    this.clearEventHandlers();

    // 卸载
    this.unmount();

    // 销毁终端
    try {
      this.terminal.dispose();
    } catch (error) {
      console.warn(`[TerminalInstance] Failed to dispose terminal for session ${this.sessionId}:`, error);
    }

    this.isDisposed = true;
    devLog('TerminalInstance', `instance disposed session=${this.sessionId}`);
  }

  /**
   * 检查实例是否已销毁
   */
  get disposed(): boolean {
    return this.isDisposed;
  }

  /**
   * 检查实例是否已挂载
   */
  get mounted(): boolean {
    return this.mountedContainer !== null;
  }

  /**
   * 获取当前挂载的容器
   */
  get container(): HTMLElement | null {
    return this.mountedContainer;
  }
}
