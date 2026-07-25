//! IPC für das Befehls-Panel: liest Skills, Agents, Slash-Commands und MCP-Connectors vom
//! Server.
//!
//! Eigene Datei statt eines weiteren Blocks in `sessions.rs` — die ist mit ~660 Zeilen bereits
//! groß genug, und mit dem Katalog hat sie inhaltlich nichts zu tun.
//!
//! Der Projektpfad kommt als Parameter vom Frontend statt hier aus der `session_id` aufgelöst zu
//! werden: die Sessionliste (`SessionInfo.cwd`) kennt ihn ohnehin schon, ein zusätzlicher
//! `tmux display -p`-Roundtrip wäre also verschenkt.

use tauri::{AppHandle, State};

use claudedeck_core::catalog::{commands as catalog_cmd, parser, Catalog};

use crate::error::ApiError;
use crate::state::AppState;

use super::sessions::{note_ssh_failure, require_conn};

/// Sammelt den Katalog in zwei Execs über die bestehende gemultiplexte Verbindung.
///
/// `project_dir` ist das Arbeitsverzeichnis der aktiven Session (`None`, wenn keine offen ist) —
/// dessen `.claude/` liefert die projektlokalen Einträge.
///
/// Beide Kommandos enden auf `|| true`: ein Server ohne `~/.claude` oder ohne `claude`-CLI ist
/// kein Fehlerfall, sondern ergibt schlicht eine leere Gruppe.
#[tauri::command]
pub async fn list_commands(
    app: AppHandle,
    state: State<'_, AppState>,
    project_dir: Option<String>,
) -> Result<Catalog, ApiError> {
    let conn = {
        let inner = state.lock().await;
        require_conn(&inner)?
    };

    let scan = conn
        .exec_capture(&catalog_cmd::cmd_scan_catalog(project_dir.as_deref()))
        .await
        .map_err(|e| note_ssh_failure(&app, e))?;

    let mcp = conn
        .exec_capture(&catalog_cmd::cmd_mcp_list())
        .await
        .map_err(|e| note_ssh_failure(&app, e))?;

    Ok(Catalog {
        entries: parser::parse_catalog(&scan.stdout, project_dir.as_deref()),
        connectors: parser::parse_mcp_list(&mcp.stdout),
    })
}
