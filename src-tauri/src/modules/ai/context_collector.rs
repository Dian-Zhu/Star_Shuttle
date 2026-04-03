use crate::modules::ai::types::TerminalContext;
use crate::modules::connection::{ConnectionManager, DefaultConnectionManager};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// 从 SSH 会话执行命令收集终端上下文
pub fn collect_terminal_context(
    manager: &Arc<RwLock<DefaultConnectionManager>>,
    session_id: Uuid,
    lines: u32,
) -> Result<TerminalContext, String> {
    let mgr = manager
        .read()
        .map_err(|e| format!("Failed to acquire read lock: {}", e))?;

    // 获取 session -> connection_id -> config
    let session = mgr
        .get_session(&session_id)
        .ok_or_else(|| format!("Session {} not found", session_id))?;

    let connection_id = session.connection_id;

    let host = mgr
        .get_connection_config(&connection_id)
        .map(|cfg| format!("{}@{}:{}", cfg.username, cfg.host, cfg.port))
        .unwrap_or_else(|| format!("session:{}", session_id));

    // 通过已建立的 SSH 会话执行命令获取上下文
    let context_cmd = format!(
        "echo '=== PWD ===' && pwd 2>/dev/null; echo '=== SHELL ===' && echo $SHELL; echo '=== UNAME ===' && uname -a 2>/dev/null; echo '=== HISTORY (last {lines}) ===' && (history {lines} 2>/dev/null || fc -l -{lines} 2>/dev/null || true)",
        lines = lines
    );

    let content = mgr
        .exec_command(&session_id, &context_cmd)
        .unwrap_or_else(|e| format!("(Unable to collect terminal context: {})", e));

    let lines_count = content.lines().count();

    Ok(TerminalContext {
        session_id,
        host,
        content,
        lines_count,
    })
}

/// 执行单条只读命令并返回输出
pub fn exec_readonly_command(
    manager: &Arc<RwLock<DefaultConnectionManager>>,
    session_id: Uuid,
    command: &str,
) -> Result<String, String> {
    let mgr = manager
        .read()
        .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
    mgr.exec_command(&session_id, command)
        .map_err(|e| e.to_string())
}
