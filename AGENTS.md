# Repository Guidelines

## Project Structure & Module Organization

This repository is a Tauri desktop app with a Svelte frontend and a Rust backend.

- `src/`: Svelte 5 + TypeScript UI, stores, and service wrappers.
- `src/components/`: UI components such as terminal, layout, connection, and file-transfer views.
- `src/lib/`: frontend services like `terminalService.ts`, `connectionService.ts`, `sftpService.ts`, and tests such as `transferQueueService.test.ts`.
- `src-tauri/src/`: Rust backend entrypoints and modules.
- `src-tauri/src/modules/connection/`: SSH, known-hosts, auth, and session management.
- `src-tauri/src/modules/sftp/`: SFTP/SCP file operations.
- `src-tauri/src/modules/db/`: SQLite persistence and settings.
- `doc/`: design, review, and deployment notes.

## Build, Test, and Development Commands

- `npm install`: install frontend dependencies.
- `npm run dev`: start the Vite frontend.
- `npx tauri dev`: run the full desktop app in development.
- `npm run build`: build the frontend bundle.
- `npx tauri build`: build the packaged desktop app.
- `npm run check`: run Svelte and TypeScript checks.
- `npm run lint`: run ESLint on `.ts` and `.svelte` files.
- `npm test`: run Vitest tests.
- `cargo test` (from `src-tauri/`): run Rust unit tests.

## Coding Style & Naming Conventions

Use TypeScript for frontend logic and Rust 2021 for backend code. Follow Prettier defaults for frontend formatting and keep Rust idiomatic.

- Components: `PascalCase.svelte`
- Frontend services/stores: `camelCase.ts`
- Rust modules: `snake_case.rs`
- Prefer descriptive command and event names such as `terminal-output-{sessionId}`.

Run `npm run format`, `npm run lint`, and `cargo test` before opening a PR.

## Testing Guidelines

Frontend tests use Vitest and are colocated as `*.test.ts` under `src/lib/`. Rust tests live inline under `#[cfg(test)]`.

- Add tests for new queueing, terminal, SFTP, or state-management behavior.
- Prefer focused unit tests over large end-to-end flows.
- When fixing a regression, add a test that fails before the fix.

## Commit & Pull Request Guidelines

Recent history uses short, release-style commit messages, often in Chinese, for example `1.16.2 修复浅色主题问题`. Keep commits concise and action-oriented.

PRs should include:

- a short summary of user-visible changes
- affected areas such as `src/lib/terminalService.ts` or `src-tauri/src/modules/sftp/`
- screenshots or recordings for UI changes
- test coverage or a brief note if tests could not run

## Security & Configuration Tips

Be careful with Tauri command exposure, app-lock behavior, file-system permissions, and SSH trust material. Changes touching `src-tauri/capabilities/default.json`, `known_hosts`, connection lifecycle, or SFTP buffering need extra review.
