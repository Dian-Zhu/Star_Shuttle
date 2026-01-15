# API Documentation

## Connection Module

### ConnectionManager

The `ConnectionManager` trait defines the core functionality for managing SSH connections and sessions.

#### Methods

- `connect(config: &ConnectionConfig) -> Result<Uuid, ConnectionError>`
  - Initiates a new SSH connection based on the provided configuration.
  - Returns a session ID on success.

- `disconnect(session_id: &Uuid) -> Result<(), ConnectionError>`
  - Disconnects an active session.

- `get_session(session_id: &Uuid) -> Option<&SessionInfo>`
  - Retrieves information about a specific session.

- `get_all_sessions() -> Vec<SessionInfo>`
  - Returns a list of all active sessions.

- `save_connection_config(config: ConnectionConfig) -> Result<(), ConnectionError>`
  - Saves a connection configuration to the persistent store.

- `delete_connection_config(connection_id: &Uuid) -> Result<(), ConnectionError>`
  - Deletes a connection configuration.

### ChannelTracker

The `ChannelTracker` struct is responsible for monitoring data transfer across SSH channels.

#### Methods

- `new() -> Self`
  - Creates a new `ChannelTracker` instance.
  - Initializes the logging directory in the system's temporary folder.

- `register_session(session_id: Uuid)`
  - Registers a new session for tracking.
  - Initializes statistics for the session.

- `log_data(session_id: Uuid, data: &[u8], direction: &str)`
  - Logs data transfer events.
  - Updates in-memory statistics (bytes sent/received, counts).
  - Appends a log entry to the session's log file.
  - `direction` should be "sent" or "received".

- `get_stats(session_id: &Uuid) -> Option<&ChannelStats>`
  - Retrieves current statistics for a session.

#### Data Structures

**ChannelStats**
```rust
pub struct ChannelStats {
    pub session_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub input_count: u64,
    pub output_count: u64,
}
```

## Terminal Module

### TerminalBuffer

The `TerminalBuffer` manages the state of the terminal screen, including the grid of cells, cursor position, and attributes.

#### Methods

- `reset()`
  - Resets the terminal to its initial state.
  - Clears the screen and resets cursor position.

- `write(data: &[u8])`
  - Processes input data and updates the buffer state.

- `resize(cols: u16, rows: u16)`
  - Resizes the terminal grid.

### TerminalParser

The `TerminalParser` handles ANSI escape sequences and control characters.

#### Supported Sequences

- **C0 Control Characters**:
  - `\n` (Line Feed): Moves cursor down.
  - `\r` (Carriage Return): Moves cursor to column 0.
  - `\b` (Backspace): Moves cursor back one position.
  - `\t` (Tab): Moves cursor to next tab stop.

- **CSI Sequences**:
  - `CSI J` (Erase in Display): Clears part or all of the screen.
  - `CSI K` (Erase in Line): Clears part or all of the line.
  - `CSI m` (SGR): Sets graphics rendition (colors, bold, etc.).
  - `CSI c` (Device Attributes): Reports terminal identity.

- **Reset**:
  - `RIS` (Reset to Initial State): Full reset of the terminal.

## Error Handling

All modules use `thiserror` for structured error handling.

- `ConnectionError`: Covers configuration, authentication, IO, and SSH protocol errors.
- `TerminalError`: Covers parsing and buffer manipulation errors.
