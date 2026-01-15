# System Deployment Guide

## Prerequisites

- **Rust Toolchain**: Stable channel (1.70+ recommended).
- **Node.js**: v18+ (for frontend build).
- **System Dependencies**:
  - `libwebkit2gtk-4.0-dev` (Linux)
  - `build-essential`
  - `curl`
  - `wget`
  - `libssl-dev`
  - `libgtk-3-dev`
  - `libayatana-appindicator3-dev`
  - `librsvg2-dev`

## Build Instructions

1.  **Clone the Repository**
    ```bash
    git clone <repository_url>
    cd Star_Shuttle
    ```

2.  **Install Frontend Dependencies**
    ```bash
    npm install
    ```

3.  **Build Frontend**
    ```bash
    npm run build
    ```
    This generates the static assets in `dist/` (or `build/`).

4.  **Build Rust Backend**
    ```bash
    cd src-tauri
    cargo build --release
    ```
    The binary will be located in `src-tauri/target/release/`.

## Running the Application

### Development Mode
To run the application with hot-reloading:
```bash
npm run tauri dev
```

### Production Mode
To run the built binary:
```bash
./src-tauri/target/release/ssh_remote_manager
```

## Configuration

The application stores configuration in the user's data directory:
- **Linux**: `~/.local/share/star_shuttle/` (or similar, depending on XDG specs)
- **Windows**: `%APPDATA%\star_shuttle\`
- **macOS**: `~/Library/Application Support/star_shuttle/`

### Logs

- **Application Logs**: Standard output/error.
- **Channel Tracking Logs**: Stored in a temporary directory (e.g., `/tmp/star_shuttle_logs/` on Linux) by default.
  - To change the log directory, modify `ChannelTracker::new()` in `src-tauri/src/modules/connection/tracking.rs`.

## Troubleshooting

- **SSH Connection Failures**:
  - Check network connectivity.
  - Verify host key fingerprints (logs will show "Server key fingerprint").
  - Ensure correct authentication credentials.

- **Terminal Display Issues**:
  - Verify the font installation (monospaced font required).
  - Check `TERM` environment variable (defaults to `xterm-256color`).

## Performance Tuning

- **Channel Tracking**:
  - The tracker writes to disk on every data packet. For high-throughput sessions, consider disabling logging or increasing buffer size in `ChannelTracker`.
- **Render Performance**:
  - The frontend uses `xterm.js` with WebGL addon (if enabled). Ensure GPU acceleration is available for best performance.
