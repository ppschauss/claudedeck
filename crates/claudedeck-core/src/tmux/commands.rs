//! Baut tmux-Kommandozeilen. Einzige Stelle im Projekt, die Shell-Strings zusammensetzt —
//! alle Werte laufen durch shell_quote, Targets sind mit `=` exakt (tmux matcht sonst Präfixe).

/// POSIX-sicheres Single-Quoting: immer gequotet, eingebettete ' als '\''.
pub fn shell_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str(r"'\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Feldtrenner für tmux `-F`-Formatstrings. Ein echter Tab wird von tmux in der
/// Listen-Ausgabe zu `_` sanitisiert (verifiziert unter tmux 3.3a und 3.5a); auch das
/// druckbare Unicode-Zeichen `␞` (U+241E) wird auf einer SSH-Exec-Session ohne
/// Locale-Forwarding (vermutlich `C`/`POSIX`-Locale) genauso zu einem einzelnen `_`
/// zusammengefasst — empirisch mit Hex-Dump gegen Isekai (tmux, echte SSH-Exec-Session)
/// verifiziert. Ein reines ASCII-Druckzeichen ist locale-unabhängig sicher: `|` kommt in
/// `session_id`/Zahlenfeldern nie vor; wo es in freien Feldern (Name, Pfad) auftreten
/// könnte, steht das Feld an letzter Stelle und wird mit `splitn` ungeteilt gelassen.
pub const FIELD_SEP: char = '|';

/// `session_id` (`#{session_id}`, z. B. `$3`) zuerst — separatorfrei, stabiler Anker für
/// `splitn`. Die Zahlenfelder folgen fest, der beliebige `session_name` steht LAST, damit
/// ein `|` darin den Parser nicht verwirrt (`splitn` lässt das letzte Feld ungeteilt).
pub fn cmd_list_sessions() -> String {
    format!(
        "tmux list-sessions -F '#{{session_id}}{FIELD_SEP}#{{session_created}}{FIELD_SEP}#{{session_attached}}{FIELD_SEP}#{{session_name}}' 2>/dev/null || true"
    )
}

/// `session_id` zuerst (Matching-Anker statt Namensvergleich), `pane_current_command`
/// (comm-Name, max. 15 Zeichen, praktisch nie mit `|`) in der Mitte, `pane_current_path`
/// (beliebig) LAST.
pub fn cmd_list_panes() -> String {
    format!(
        "tmux list-panes -a -F '#{{session_id}}{FIELD_SEP}#{{pane_current_command}}{FIELD_SEP}#{{pane_current_path}}' 2>/dev/null || true"
    )
}

pub fn cmd_new_detached(name: &str, cwd: &str, command: &str) -> String {
    format!(
        "tmux new-session -A -d -s {} -c {} {}",
        shell_quote(name),
        shell_quote(cwd),
        shell_quote(command)
    )
}

pub fn cmd_attach(name: &str) -> String {
    format!("tmux attach -t {}", shell_quote(&format!("={name}")))
}

pub fn cmd_kill(name: &str) -> String {
    format!("tmux kill-session -t {}", shell_quote(&format!("={name}")))
}

pub fn cmd_has_session(name: &str) -> String {
    format!("tmux has-session -t {}", shell_quote(&format!("={name}")))
}

pub fn cmd_pane_cwd(name: &str) -> String {
    format!(
        "tmux display -p -t {} '#{{pane_current_path}}'",
        shell_quote(&format!("={name}"))
    )
}

pub fn cmd_scan_projects(paths: &[String]) -> String {
    let quoted: Vec<String> = paths.iter().map(|p| shell_quote(p)).collect();
    format!(
        "find {} -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort",
        quoted.join(" ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_einfacher_string() {
        assert_eq!(shell_quote("abc"), "'abc'");
    }

    #[test]
    fn quote_mit_leerzeichen_und_dollar() {
        assert_eq!(shell_quote("a b$c"), "'a b$c'");
    }

    #[test]
    fn quote_mit_single_quote() {
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn new_detached_quotet_alles_und_nutzt_a_d() {
        assert_eq!(
            cmd_new_detached("cc-x", "/mnt/cache/app data", "claude"),
            "tmux new-session -A -d -s 'cc-x' -c '/mnt/cache/app data' 'claude'"
        );
    }

    #[test]
    fn attach_nutzt_exaktes_target() {
        assert_eq!(cmd_attach("cc-x"), "tmux attach -t '=cc-x'");
    }

    #[test]
    fn kill_nutzt_exaktes_target() {
        assert_eq!(cmd_kill("cc-x"), "tmux kill-session -t '=cc-x'");
    }

    #[test]
    fn has_session_nutzt_exaktes_target() {
        assert_eq!(cmd_has_session("cc-x"), "tmux has-session -t '=cc-x'");
    }

    #[test]
    fn pane_cwd_nutzt_exaktes_target() {
        assert_eq!(
            cmd_pane_cwd("cc-x"),
            "tmux display -p -t '=cc-x' '#{pane_current_path}'"
        );
    }

    #[test]
    fn list_sessions_nutzt_ascii_pipe_und_id_zuerst_name_last() {
        assert_eq!(FIELD_SEP, '|');
        assert_eq!(
            cmd_list_sessions(),
            "tmux list-sessions -F '#{session_id}|#{session_created}|#{session_attached}|#{session_name}' 2>/dev/null || true"
        );
    }

    #[test]
    fn list_panes_nutzt_ascii_pipe_und_id_zuerst_pfad_last() {
        assert_eq!(
            cmd_list_panes(),
            "tmux list-panes -a -F '#{session_id}|#{pane_current_command}|#{pane_current_path}' 2>/dev/null || true"
        );
    }

    #[test]
    fn scan_projects_joint_mehrere_pfade() {
        assert_eq!(
            cmd_scan_projects(&["/mnt/a".into(), "/mnt/b c".into()]),
            "find '/mnt/a' '/mnt/b c' -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort"
        );
    }
}
