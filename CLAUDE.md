# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Star Shuttle is a cross-platform SSH connection manager built with Tauri 2.x. It provides terminal access, file transfer (SFTP/SCP), and connection management through a desktop application.

**Tech Stack:**
- Frontend: Svelte 5 + TypeScript + Tailwind CSS + Vite
- Backend: Rust (Tauri 2.x)
- Terminal: xterm.js 6.0 with WebGL renderer
- SSH: russh/russh-sftp libraries
- Database: SQLite (rusqlite)

## Build Commands

```bash
# Install dependencies
npm install

# Development (frontend + Tauri dev server)
npm run dev              # Starts Vite dev server
npx tauri dev            # Starts full Tauri development mode

# Build for production
npm run build            # Build frontend
npx tauri build          # Build complete application

# Code quality
npm run check            # Svelte type checking
npm run lint             # ESLint
npm run format           # Prettier formatting

# Rust-specific (from src-tauri/)
cargo build              # Build Rust backend
cargo check              # Type check Rust code
cargo clippy             # Rust linting
```

## Architecture

### Frontend (`src/`)

- `App.svelte` - Root component, theme management
- `components/Layout.svelte` - Main layout with sidebar, terminal manager, modals
- `components/TerminalManager.svelte` - Multi-terminal tab management with split-screen support
- `components/TerminalView.svelte` - Individual xterm.js terminal instance
- `lib/store.ts` - Svelte stores for global state (connections, terminals, settings)
- `lib/terminalService.ts` - Terminal lifecycle, output buffering, xterm integration
- `lib/connectionService.ts` - Tauri command wrappers for SSH operations

### Backend (`src-tauri/src/`)

- `lib.rs` - Tauri command handlers (connect, disconnect, terminal ops, SFTP, etc.)
- `modules/connection/` - SSH connection management, known hosts, keyboard-interactive auth
- `modules/terminal/` - PTY handling, terminal I/O
- `modules/sftp/` - SFTP/SCP file operations
- `modules/db/` - SQLite persistence for connections, settings, command snippets
- `modules/credential/` - Secure credential storage via system keyring

### Frontend-Backend Communication

Tauri commands are invoked from TypeScript via `@tauri-apps/api`. Key patterns:
- Connection state managed by `DefaultConnectionManager` (Rust) with `Arc<RwLock<>>` for thread safety
- Terminal output sent via Tauri events (`terminal-output-{sessionId}`)
- SFTP operations exposed as individual Tauri commands (`sftp_ls`, `sftp_read`, etc.)

## Key Implementation Details

### Terminal Output Handling

The terminal uses adaptive batching for performance:
- Output chunks buffered and flushed based on performance metrics
- xterm 6.0 uses Promise-based `term.write()` API (not callbacks)
- WebGL renderer preferred with canvas fallback

### Connection Protocols

- SSH: Full terminal + SFTP support via russh
- RDP: Launches system RDP client with temp .rdp file

### State Management

- `activeTerminals` store tracks all terminal instances
- `selectedTerminalIndex` controls which terminal is visible
- Split-screen managed via `splitMode` and `splitTerminals` in store

## Debugging

See `DEBUGGING.md` for terminal rendering issues. Key log prefixes:
- `[TermInit]` - Terminal initialization
- `[TermOutput]` - Output event handling
- `[TerminalView]` - Visibility/resize events
