//! Katalog der auf dem Server verfügbaren Claude-Code-Befehle: Skills, Agents, Slash-Commands
//! und MCP-Connectors.
//!
//! Zwei Quellen, beide über die bestehende gemultiplexte SSH-Verbindung (`ssh::connection::
//! SshConnection::exec_capture`):
//!
//! 1. **Dateien** unter `~/.claude/` und im `.claude/` des aktiven Projekts — ein einzelner
//!    Sammel-Exec liefert Pfad plus Frontmatter-Kopf je Datei, [`parser::parse_catalog`] macht
//!    daraus Einträge.
//! 2. **Connectors** über `claude mcp list` statt `~/.claude.json` zu parsen: die CLI liefert
//!    zusätzlich den Verbindungsstatus, und die 51 KB große `.claude.json` enthält History und
//!    Tokens, die hier nichts zu suchen haben.
//!
//! Wie bei `tmux` liegt der Shell-String-Bau in `commands.rs` und das Parsen in `parser.rs` —
//! beide rein und ohne Netz unit-testbar.

pub mod commands;
pub mod parser;

use serde::Serialize;

/// Woher ein Eintrag stammt — bestimmt die Gruppe im Befehls-Panel.
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CommandKind {
    Skill,
    Agent,
    Command,
}

/// Global (`~/.claude`) oder nur in der aktiven Session verfügbar (`<projekt>/.claude`).
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CommandScope {
    Global,
    Project,
}

/// Ein aufrufbarer Eintrag. `name` ist ohne führenden Schrägstrich gespeichert — den setzt die
/// UI beim Einfügen, weil Agents anders als Skills nicht als `/name` aufgerufen werden.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandEntry {
    pub kind: CommandKind,
    pub name: String,
    pub description: String,
    pub scope: CommandScope,
}

/// Ein MCP-Server aus `claude mcp list`.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Connector {
    pub name: String,
    pub url: String,
    /// Rohtext des Status, wie die CLI ihn meldet (z. B. „Connected", „Needs authentication").
    pub status: String,
    pub connected: bool,
}

/// Vollständiges Ergebnis eines `list_commands`-Aufrufs.
#[derive(Serialize, Debug, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub entries: Vec<CommandEntry>,
    pub connectors: Vec<Connector>,
}
