<script lang="ts">
  import { onMount, onDestroy, createEventDispatcher } from 'svelte'
  import { Terminal } from '@xterm/xterm'
  import { FitAddon } from '@xterm/addon-fit'
  import { SearchAddon } from '@xterm/addon-search'
  import { settings, getXtermTheme, getBaseXtermTheme, type Connection } from '../../lib/store'
  import { terminalPool } from '../../lib/terminalPool'
  import type { TerminalProxy } from '../../lib/terminalProxy'
  import ContextMenu from '../ui/ContextMenu.svelte'
  import ContextMenuItem from '../ui/ContextMenuItem.svelte'
  import ContextMenuDivider from '../ui/ContextMenuDivider.svelte'
  import {
    initDetachedTerminal,
    handleTerminalInput,
    sendTerminalResize,
    calculateBrightness,
  } from '../../lib/terminalService'
  import { attachAlternateBufferPreserver } from '../../lib/terminalAltBuffer'
  import TerminalIcon from '../icons/TerminalIcon.svelte'
  import XIcon from '../icons/XIcon.svelte'

  export let sessionId: string
  export let connection: Connection
  export let isRoot: boolean = false
  export let paneIndex: number = 1
  export let onInit: ((proxy: TerminalProxy) => void) | undefined = undefined
  export let onFocus: (() => void) | undefined = undefined

  export let isVisible: boolean = true
  export let shouldRestoreFocus: boolean = false

  const dispatch = createEventDispatcher<{
    split: { direction: 'horizontal' | 'vertical' }
    close: void
    active: void
  }>()

  let container: HTMLElement
  let terminal: Terminal | null = null
  let fitAddon: FitAddon | null = null
  let searchAddon: SearchAddon | null = null
  let resizeObserver: ResizeObserver | null = null
  let isInitialized = false
  let isDestroyed = false
  let mountVersion = 0
  let focusListener: (() => void) | null = null
  let layoutFrame: number | null = null
  let layoutFrameMode: 'raf-outer' | 'raf-inner' | 'timeout' | null = null
  let layoutRefreshPending = false
  let layoutFocusPending = false
  let lastLayoutSize = { width: 0, height: 0 }
  let lastTerminalGrid = { cols: 0, rows: 0 }
  let lastVisibilityState = isVisible
  let mouseDownListener: (() => void) | null = null
  let textareaEl: HTMLTextAreaElement | null = null
  let titleChangeDisposable: { dispose: () => void } | null = null
  let altBufferPreserverDisposable: { dispose: () => void } | null = null
  let ctrlCArmedUntil = 0

  const CTRL_C_INTERRUPT_WINDOW_MS = 800

  // Search state
  let showSearch = false
  let searchTerm = ''
  $: searchInputId = `search-input-${sessionId}`

  // Context Menu state
  let contextMenu = {
    show: false,
    x: 0,
    y: 0,
  }

  function isCopyShortcut(e: KeyboardEvent) {
    const key = e.key.toLowerCase()
    if (e.metaKey && key === 'c') return true
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && key === 'c') return true
    if (e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey && key === 'c') return true
    return false
  }

  function isPlainCtrlC(e: KeyboardEvent) {
    return e.ctrlKey && !e.metaKey && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'c'
  }

  function copyTerminalSelection(term: Terminal): boolean {
    const selection = term.getSelection() ?? ''
    if (!selection) return false
    if (!navigator.clipboard?.writeText) return false
    void navigator.clipboard.writeText(selection)
    return true
  }

  function getTerminalSelection(): string {
    return terminal?.getSelection() ?? ''
  }

  function armCtrlCInterrupt() {
    ctrlCArmedUntil = Date.now() + CTRL_C_INTERRUPT_WINDOW_MS
  }

  function disarmCtrlCInterrupt() {
    ctrlCArmedUntil = 0
  }

  function handlePaste(e: ClipboardEvent) {
    const text = e.clipboardData?.getData('text/plain') ?? ''
    if (!text) return
    e.preventDefault()
    e.stopPropagation()

    if (!sessionId) return
    handleTerminalInput(sessionId, text, connection)
    setTimeout(() => terminal?.focus(), 0)
  }

  function attachTerminalKeybindings(term: Terminal) {
    term.attachCustomKeyEventHandler((e: KeyboardEvent) => {
      if (e.type === 'keydown' && isCopyShortcut(e)) {
        if (isPlainCtrlC(e)) {
          const now = Date.now()
          if (now <= ctrlCArmedUntil) {
            disarmCtrlCInterrupt()
            if (sessionId) {
              handleTerminalInput(sessionId, '\x03', connection)
            }
            return false
          }

          copyTerminalSelection(term)
          armCtrlCInterrupt()
          return false
        }

        if (!copyTerminalSelection(term)) return true
        disarmCtrlCInterrupt()
        return false
      }
      if (e.type === 'keydown') {
        disarmCtrlCInterrupt()
      }
      if ((e.ctrlKey || e.metaKey) && e.key === 'f' && e.type === 'keydown') {
        showSearch = !showSearch
        if (showSearch) {
          setTimeout(() => document.getElementById(searchInputId)?.focus(), 0)
        } else {
          term.focus()
        }
        return false
      }
      return true
    })
  }

  function resolvePooledTerminal() {
    const instance = terminalPool.retrieveInstance(sessionId)
    if (!instance || instance.disposed) return null
    return instance
  }

  function cancelScheduledLayoutWork() {
    if (layoutFrame !== null) {
      if (
        (layoutFrameMode === 'raf-outer' || layoutFrameMode === 'raf-inner') &&
        typeof cancelAnimationFrame === 'function'
      ) {
        cancelAnimationFrame(layoutFrame)
      } else {
        clearTimeout(layoutFrame)
      }
    }
    layoutFrame = null
    layoutFrameMode = null
    layoutRefreshPending = false
    layoutFocusPending = false
  }

  function scheduleLayoutWork(options: { refresh?: boolean; focus?: boolean } = {}) {
    if (!terminal || !fitAddon || !isInitialized || !isVisible) return
    layoutRefreshPending = layoutRefreshPending || Boolean(options.refresh)
    layoutFocusPending = layoutFocusPending || Boolean(options.focus)
    if (layoutFrame !== null) return

    const run = () => {
      layoutFrame = null
      layoutFrameMode = null
      if (!terminal || !fitAddon || !isInitialized || !isVisible) {
        layoutRefreshPending = false
        layoutFocusPending = false
        return
      }

      fitAddon.fit()
      if (lastTerminalGrid.cols !== terminal.cols || lastTerminalGrid.rows !== terminal.rows) {
        lastTerminalGrid = { cols: terminal.cols, rows: terminal.rows }
        sendTerminalResize(sessionId, terminal.cols, terminal.rows)
      }
      if (layoutRefreshPending) {
        terminal.refresh(0, terminal.rows - 1)
      }
      if (layoutFocusPending) {
        terminal.focus()
      }
      layoutRefreshPending = false
      layoutFocusPending = false
    }

    if (typeof requestAnimationFrame === 'function') {
      layoutFrameMode = 'raf-outer'
      layoutFrame = requestAnimationFrame(() => {
        layoutFrameMode = 'raf-inner'
        layoutFrame = requestAnimationFrame(run)
      })
      return
    }

    layoutFrameMode = 'timeout'
    layoutFrame = window.setTimeout(run, 0) as unknown as number
  }

  // Initialization
  onMount(async () => {
    isDestroyed = false
    const currentMountVersion = ++mountVersion

    resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0]
      const width = Math.round(entry?.contentRect.width ?? container?.clientWidth ?? 0)
      const height = Math.round(entry?.contentRect.height ?? container?.clientHeight ?? 0)
      if (width <= 0 || height <= 0) return
      if (lastLayoutSize.width === width && lastLayoutSize.height === height) return
      lastLayoutSize = { width, height }
      scheduleLayoutWork()
    })

    const pooled = resolvePooledTerminal()
    if (pooled) {
      terminal = pooled.terminal
      fitAddon = pooled.fitAddon
      searchAddon = pooled.searchAddon
      pooled.mount(container)
      attachTerminalKeybindings(terminal)

      if (onInit) {
        onInit(terminalPool.getProxy(sessionId))
      }
    } else {
      // Initialize new detached terminal
      const result = await initDetachedTerminal(container, sessionId, connection)
      if (isDestroyed || currentMountVersion !== mountVersion) {
        return
      }
      if (!result) {
        return
      }

      terminal = result.terminal
      fitAddon = result.fitAddon
      searchAddon = result.searchAddon
      attachTerminalKeybindings(terminal)

      if (onInit) onInit(terminalPool.getProxy(sessionId))
    }

    if (isDestroyed || currentMountVersion !== mountVersion) {
      return
    }

    if (terminal) {
      titleChangeDisposable = terminal.onTitleChange(() => {
        // Use title change as a proxy for activity/focus if needed,
        // but better to use onFocus event from xterm textarea
      })
      altBufferPreserverDisposable = attachAlternateBufferPreserver(terminal)

      focusListener = () => {
        if (onFocus) onFocus()
        dispatch('active')
      }
      mouseDownListener = () => {
        if (onFocus) onFocus()
        dispatch('active')
      }

      textareaEl = terminal.textarea ?? null
      textareaEl?.addEventListener('focus', focusListener)
      container.addEventListener('mousedown', mouseDownListener)
    }

    if (container) {
      resizeObserver.observe(container)
      container.addEventListener('paste', handlePaste, true)
    }

    isInitialized = true
  })

  onDestroy(() => {
    isDestroyed = true
    mountVersion += 1

    if (resizeObserver) {
      resizeObserver.disconnect()
    }
    cancelScheduledLayoutWork()
    if (titleChangeDisposable) {
      try {
        titleChangeDisposable.dispose()
      } catch {
        // Ignore disposal failures during teardown.
      }
      titleChangeDisposable = null
    }
    if (altBufferPreserverDisposable) {
      try {
        altBufferPreserverDisposable.dispose()
      } catch {
        // Ignore disposal failures during teardown.
      }
      altBufferPreserverDisposable = null
    }
    if (textareaEl && focusListener) {
      textareaEl.removeEventListener('focus', focusListener)
    }
    if (container && mouseDownListener) {
      container.removeEventListener('mousedown', mouseDownListener)
    }
    textareaEl = null
    focusListener = null
    mouseDownListener = null
    container?.removeEventListener('paste', handlePaste, true)

    const pooled = resolvePooledTerminal()
    if (pooled && pooled.container === container) {
      pooled.unmount()
    }

    // We NO LONGER dispose/disconnect here.
    // Session cleanup is now managed explicitly by the parent view or closeSplitSession.
    // This allows the component to be unmounted/remounted during layout changes without killing the session.
  })

  // Reactive settings updates
  $: if (terminal && isInitialized) {
    terminal.options.fontSize = $settings.terminal.fontSize
    terminal.options.fontFamily = $settings.terminal.fontFamily
    terminal.options.cursorBlink = $settings.terminal.cursorBlink
    terminal.options.cursorStyle = $settings.terminal.cursorStyle
    ;(terminal.options as any).cursorWidth = 1
    terminal.options.scrollback = $settings.terminal.scrollback
    terminal.options.theme = getXtermTheme($settings)

    const baseTheme = getBaseXtermTheme($settings)
    const bgBrightness = calculateBrightness(baseTheme.background || '#000000')
    const isLightTheme = bgBrightness > 128
    terminal.options.fontWeight = isLightTheme ? '600' : 'normal'
    terminal.options.fontWeightBold = isLightTheme ? 'bold' : 'bold'

    scheduleLayoutWork({ refresh: true })
  }

  $: if (terminal && isInitialized) {
    if (isVisible && !lastVisibilityState) {
      scheduleLayoutWork({ refresh: true, focus: shouldRestoreFocus })
    }
    if (!isVisible && lastVisibilityState) {
      cancelScheduledLayoutWork()
    }
    lastVisibilityState = isVisible
  }

  // Context Menu Handlers
  function openContextMenu(e: MouseEvent) {
    e.preventDefault()
    contextMenu.x = e.clientX
    contextMenu.y = e.clientY
    contextMenu.show = true
  }

  function closeContextMenu() {
    contextMenu.show = false
  }

  function handleMenuCopy() {
    if (!terminal) return
    copyTerminalSelection(terminal)
    disarmCtrlCInterrupt()
    closeContextMenu()
  }

  function handleSendSelectionToAi() {
    const selection = getTerminalSelection()
    if (!selection.trim()) return

    window.dispatchEvent(new CustomEvent('ai:open-with-context', { detail: selection }))
    closeContextMenu()
  }

  async function handleMenuPaste() {
    try {
      const text = await navigator.clipboard.readText()
      if (text && sessionId) {
        handleTerminalInput(sessionId, text, connection)
      }
    } catch (err) {
      console.error('Failed to paste:', err)
    }
    closeContextMenu()
    setTimeout(() => terminal?.focus(), 0)
  }

  function handleClearScreen() {
    terminal?.clear()
    closeContextMenu()
  }

  function handleSelectAll() {
    terminal?.selectAll()
    closeContextMenu()
  }

  function handleFind() {
    showSearch = true
    setTimeout(() => document.getElementById(searchInputId)?.focus(), 0)
    closeContextMenu()
  }

  function handleClearScrollback() {
    terminal?.clear()
    closeContextMenu()
  }

  function handleReset() {
    terminal?.reset()
    closeContextMenu()
  }

  function handleSplitHorizontal() {
    dispatch('split', { direction: 'horizontal' })
    closeContextMenu()
  }

  function handleSplitVertical() {
    dispatch('split', { direction: 'vertical' })
    closeContextMenu()
  }

  function handleClosePane() {
    dispatch('close')
    closeContextMenu()
  }

  // Search Handlers
  function handleSearchInput() {
    if (searchAddon) {
      searchAddon.findNext(searchTerm, {
        regex: false,
        wholeWord: false,
        caseSensitive: false,
        incremental: true,
      })
    }
  }

  function handleSearchPrevious() {
    searchAddon?.findPrevious(searchTerm, { regex: false, wholeWord: false, caseSensitive: false })
  }

  function handleSearchNext() {
    searchAddon?.findNext(searchTerm, { regex: false, wholeWord: false, caseSensitive: false })
  }

  function closeSearch() {
    showSearch = false
    terminal?.focus()
  }

  function handleSearchKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      if (e.shiftKey) handleSearchPrevious()
      else handleSearchNext()
    } else if (e.key === 'Escape') {
      closeSearch()
    }
  }
</script>

<div class="flex flex-col w-full h-full overflow-hidden group relative bg-app-bg">
  <!-- Terminal Header -->
  <div
    class="flex items-center justify-start h-[24px] select-none flex-shrink-0 relative z-10 px-2"
    style="background-color: transparent;"
  >
    <div
      class="flex items-center gap-2 rounded-full px-2.5 py-0.5 border border-app-border/50 max-w-[90%]"
      style="background-color: transparent;"
    >
      <div class="flex items-center gap-2 min-w-0">
        <TerminalIcon class="w-3.5 h-3.5 text-app-text-secondary flex-shrink-0" />
        <span
          class="text-xs text-app-text truncate font-medium max-w-[200px]"
          title={connection.name}
        >
          {#if paneIndex > 1}
            {paneIndex}:
          {/if}{connection.name}
        </span>
      </div>
      <button
        class="ml-1 p-0.5 rounded-full hover:bg-app-surface text-app-text-secondary hover:text-red-500 transition-colors flex items-center justify-center"
        on:click={handleClosePane}
        title="关闭"
      >
        <XIcon class="w-3.5 h-3.5" />
      </button>
    </div>
  </div>

  <div class="relative flex-1 w-full min-h-0 overflow-hidden">
    <div
      bind:this={container}
      class="relative z-0 w-full h-full overflow-hidden"
      on:contextmenu|preventDefault={openContextMenu}
      role="button"
      tabindex="0"
    ></div>

    <!-- Search Bar -->
    {#if showSearch}
      <div
        class="absolute top-2 right-2 z-10 bg-app-surface border border-app-border shadow-lg rounded-md p-1.5 flex items-center gap-1.5"
      >
        <input
          id={searchInputId}
          type="text"
          bind:value={searchTerm}
          on:input={handleSearchInput}
          on:keydown={handleSearchKeydown}
          placeholder="Find..."
          class="w-48 px-2 py-1 text-xs bg-app-bg border border-app-border rounded text-app-text focus:outline-none focus:border-primary-500"
        />
        <button
          class="p-1 hover:bg-app-bg-hover rounded text-app-text-secondary"
          aria-label="上一个匹配"
          on:click={handleSearchPrevious}
        >
          <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"
            ><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 15l7-7 7 7"
            ></path></svg
          >
        </button>
        <button
          class="p-1 hover:bg-app-bg-hover rounded text-app-text-secondary"
          aria-label="下一个匹配"
          on:click={handleSearchNext}
        >
          <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"
            ><path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M19 9l-7 7-7-7"
            ></path></svg
          >
        </button>
        <button
          class="p-1 hover:bg-app-bg-hover rounded text-app-text-secondary"
          aria-label="关闭查找"
          on:click={closeSearch}
        >
          <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"
            ><path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M6 18L18 6M6 6l12 12"
            ></path></svg
          >
        </button>
      </div>
    {/if}

    <!-- Context Menu -->
    {#if contextMenu.show}
      <ContextMenu x={contextMenu.x} y={contextMenu.y} on:close={closeContextMenu}>
        <ContextMenuItem on:click={handleMenuCopy} label="复制">
          <span slot="right">Ctrl+C / Ctrl+Shift+C</span>
        </ContextMenuItem>
        <ContextMenuItem
          on:click={handleSendSelectionToAi}
          label="发送选中内容到 AI"
          disabled={!getTerminalSelection().trim()}
        />
        <ContextMenuItem on:click={handleMenuPaste} label="粘贴">
          <span slot="right">Ctrl+Shift+V</span>
        </ContextMenuItem>
        <ContextMenuDivider />
        <ContextMenuItem on:click={handleClearScreen} label="清屏" />
        <ContextMenuItem on:click={handleSelectAll} label="全选" />
        <ContextMenuItem on:click={handleFind} label="查找">
          <span slot="right">Ctrl+Shift+F</span>
        </ContextMenuItem>
        <ContextMenuDivider />
        <ContextMenuItem on:click={handleSplitHorizontal} label="上下分屏">
          <svg slot="icon" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"
            ><path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M4 6h16M4 12h16M4 18h16"
            ></path></svg
          >
        </ContextMenuItem>
        <ContextMenuItem on:click={handleSplitVertical} label="左右分屏">
          <svg slot="icon" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"
            ><path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M8 4h13M8 20h13M3 4h.01M3 20h.01"
            ></path></svg
          >
        </ContextMenuItem>
        {#if !isRoot}
          <ContextMenuItem on:click={handleClosePane} label="关闭分屏" danger />
        {/if}
        <ContextMenuDivider />
        <ContextMenuItem on:click={handleClearScrollback} label="清除滚动缓冲区" />
        <ContextMenuItem on:click={handleReset} label="重置终端" danger />
      </ContextMenu>
    {/if}
  </div>
</div>
