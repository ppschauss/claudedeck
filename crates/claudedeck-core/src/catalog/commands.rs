//! Baut die Shell-Kommandos für die Katalog-Discovery.
//!
//! Wie bei `tmux::commands` laufen alle Werte durch `shell_quote` — Projektpfade kommen aus
//! `pane_current_path` und sind damit Fremdeingabe.

use crate::tmux::commands::{shell_quote, LOCALE_PREFIX};

/// Trennmarke vor jedem Dateipfad im Sammel-Output. Bewusst ein Präfix am Zeilenanfang statt
/// eines Feldtrenners: Frontmatter enthält beliebige Zeichen, aber keine Zeile beginnt mit
/// dieser Marke.
pub const FILE_MARKER: &str = "===F:";

/// Wie viele Bytes je Datei übertragen werden. Das Frontmatter steht immer am Dateianfang;
/// mehr zu holen würde bei großen SKILL.md nur die Verbindung belasten.
const HEAD_BYTES: usize = 2048;

/// Wie tief unter `~/.claude/plugins/cache` nach `SKILL.md` gesucht wird. Der Pfad ist
/// `<plugin-repo>/<plugin>/<version>/skills/<name>/SKILL.md`, also normalerweise sechs Ebenen —
/// die Grenze verhindert, dass `find` in tiefe Plugin-Bäume läuft.
const PLUGIN_MAX_DEPTH: u8 = 8;

/// Sammelt Skills, Agents und Slash-Commands in **einem** Exec.
///
/// Ausgabeformat je Datei: eine Zeile `===F:<pfad>`, danach die ersten [`HEAD_BYTES`] Bytes.
/// Fehlende Verzeichnisse sind der Normalfall (nicht jedes Projekt hat ein `.claude/`) und
/// werden über `2>/dev/null` still übergangen; `|| true` hält den Exit-Code bei 0, damit
/// `exec_capture` das nicht als Fehler wertet.
pub fn cmd_scan_catalog(project_dir: Option<&str>) -> String {
    let mut finds = vec![
        // Globale Skills: ~/.claude/skills/<name>/SKILL.md
        "find \"$HOME/.claude/skills\" -mindepth 2 -maxdepth 2 -name SKILL.md 2>/dev/null".to_string(),
        // Plugin-Skills liegen mehrere Ebenen tiefer, immer unter einem skills/-Segment.
        format!(
            "find \"$HOME/.claude/plugins/cache\" -maxdepth {PLUGIN_MAX_DEPTH} -path '*/skills/*' -name SKILL.md 2>/dev/null"
        ),
        "find \"$HOME/.claude/agents\" -maxdepth 1 -name '*.md' 2>/dev/null".to_string(),
        "find \"$HOME/.claude/commands\" -maxdepth 1 -name '*.md' 2>/dev/null".to_string(),
    ];

    if let Some(dir) = project_dir {
        let quoted = shell_quote(dir);
        finds.push(format!(
            "find {quoted}/.claude/skills -mindepth 2 -maxdepth 2 -name SKILL.md 2>/dev/null"
        ));
        finds.push(format!(
            "find {quoted}/.claude/agents -maxdepth 1 -name '*.md' 2>/dev/null"
        ));
        finds.push(format!(
            "find {quoted}/.claude/commands -maxdepth 1 -name '*.md' 2>/dev/null"
        ));
    }

    format!(
        "{LOCALE_PREFIX} sh -c '{{ {} ; }} | while IFS= read -r f; do printf \"{FILE_MARKER}%s\\n\" \"$f\"; head -c {HEAD_BYTES} \"$f\"; printf \"\\n\"; done' 2>/dev/null || true",
        finds.join(" ; ")
    )
}

/// Liest die Connector-Liste über die CLI statt über `~/.claude.json` — siehe Modulkommentar.
pub fn cmd_mcp_list() -> String {
    format!("{LOCALE_PREFIX} claude mcp list 2>/dev/null || true")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_ohne_projekt_sucht_nur_global() {
        let cmd = cmd_scan_catalog(None);
        assert!(cmd.contains("$HOME/.claude/skills"));
        assert!(cmd.contains("$HOME/.claude/agents"));
        assert!(cmd.contains("$HOME/.claude/commands"));
        assert!(cmd.contains("plugins/cache"));
        assert!(!cmd.contains("/.claude/skills -mindepth 2 -maxdepth 2 -name SKILL.md 2>/dev/null'"));
    }

    #[test]
    fn scan_mit_projekt_ergaenzt_projektpfade() {
        let cmd = cmd_scan_catalog(Some("/mnt/cache/appdata/claudedeck"));
        assert!(cmd.contains("'/mnt/cache/appdata/claudedeck'/.claude/skills"));
        assert!(cmd.contains("'/mnt/cache/appdata/claudedeck'/.claude/agents"));
        assert!(cmd.contains("'/mnt/cache/appdata/claudedeck'/.claude/commands"));
    }

    /// Projektpfade stammen aus `pane_current_path` und sind damit Fremdeingabe.
    #[test]
    fn scan_quotet_projektpfade_mit_sonderzeichen() {
        let cmd = cmd_scan_catalog(Some("/mnt/mein projekt"));
        assert!(cmd.contains("'/mnt/mein projekt'/.claude/skills"));
    }

    #[test]
    fn scan_setzt_locale_prefix_und_endet_fehlertolerant() {
        let cmd = cmd_scan_catalog(None);
        assert!(cmd.starts_with(LOCALE_PREFIX));
        assert!(cmd.ends_with("|| true"));
    }

    #[test]
    fn scan_nutzt_die_marke_aus_der_konstante() {
        assert!(cmd_scan_catalog(None).contains(FILE_MARKER));
    }

    #[test]
    fn mcp_list_ist_fehlertolerant() {
        let cmd = cmd_mcp_list();
        assert!(cmd.starts_with(LOCALE_PREFIX));
        assert!(cmd.contains("claude mcp list"));
        assert!(cmd.ends_with("|| true"));
    }
}
