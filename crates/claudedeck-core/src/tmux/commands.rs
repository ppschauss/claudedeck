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

/// Env-Prefix für jedes tmux-Kommando, das Text überträgt.
///
/// Die SSH-Exec-Session kommt ohne Locale-Forwarding (`request_pty` in `ssh/pty.rs` setzt keine
/// Env-Variablen, und sshd lehnt fremde Variablen per `AcceptEnv` üblicherweise ab) — sie läuft
/// also in `C`/`POSIX`. Dort zeichnet tmux keine Rahmenzeichen und Readline verstümmelt
/// 8-Bit-Eingaben, was Umlaute in *beide* Richtungen zerstört. Deshalb wird die Locale hier im
/// Kommandostring gesetzt statt per `set_env`.
///
/// `C.UTF-8` und nicht `de_DE.UTF-8`: Erstere existiert auf glibc und musl ohne vorheriges
/// `locale-gen`, letztere muss auf dem Zielsystem erst erzeugt worden sein.
pub const LOCALE_PREFIX: &str = "LANG=C.UTF-8 LC_ALL=C.UTF-8";

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
        "{LOCALE_PREFIX} tmux list-sessions -F '#{{session_id}}{FIELD_SEP}#{{session_created}}{FIELD_SEP}#{{session_attached}}{FIELD_SEP}#{{session_name}}' 2>/dev/null || true"
    )
}

/// `session_id` zuerst (Matching-Anker statt Namensvergleich), `pane_current_command`
/// (comm-Name, max. 15 Zeichen, praktisch nie mit `|`) in der Mitte, `pane_current_path`
/// (beliebig) LAST.
pub fn cmd_list_panes() -> String {
    format!(
        "{LOCALE_PREFIX} tmux list-panes -a -F '#{{session_id}}{FIELD_SEP}#{{pane_current_command}}{FIELD_SEP}#{{pane_current_path}}' 2>/dev/null || true"
    )
}

pub fn cmd_new_detached(name: &str, cwd: &str, command: &str) -> String {
    format!(
        "{LOCALE_PREFIX} tmux new-session -A -d -s {} -c {} {}",
        shell_quote(name),
        shell_quote(cwd),
        shell_quote(command)
    )
}

/// `-u` erzwingt UTF-8 im tmux-*Client* — unabhängig davon, ob tmux die Locale erkennt. Ohne das
/// zeichnet Claude Codes TUI keine Rahmenzeichen (siehe [`LOCALE_PREFIX`]).
pub fn cmd_attach(name: &str) -> String {
    format!(
        "{LOCALE_PREFIX} tmux -u attach -t {}",
        shell_quote(&format!("={name}"))
    )
}

pub fn cmd_kill(name: &str) -> String {
    format!("tmux kill-session -t {}", shell_quote(&format!("={name}")))
}

pub fn cmd_has_session(name: &str) -> String {
    format!("tmux has-session -t {}", shell_quote(&format!("={name}")))
}

pub fn cmd_pane_cwd(name: &str) -> String {
    format!(
        "{LOCALE_PREFIX} tmux display -p -t {} '#{{pane_current_path}}'",
        shell_quote(&format!("={name}"))
    )
}

/// Baut den `claude`-Aufruf für eine neu gestartete Session.
///
/// Beide Flags sind laut `claude --help` (2.1.220) dokumentiert: `--model` nimmt einen Alias
/// (`opus`, `sonnet`, `fable`) oder einen vollen Namen, `--effort` eine der Stufen
/// `low|medium|high|xhigh|max`. Nicht gesetzte oder leere Werte lassen das jeweilige Flag weg,
/// sodass Claude Code seine eigenen Vorgaben behält.
///
/// Die Werte stammen aus der von Hand editierbaren `config.json` und laufen deshalb durch
/// [`shell_quote`].
pub fn claude_invocation(model: Option<&str>, effort: Option<&str>) -> String {
    let mut cmd = String::from("claude");
    for (flag, value) in [("--model", model), ("--effort", effort)] {
        if let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) {
            cmd.push_str(&format!(" {flag} {}", shell_quote(value)));
        }
    }
    cmd
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
            "LANG=C.UTF-8 LC_ALL=C.UTF-8 tmux new-session -A -d -s 'cc-x' -c '/mnt/cache/app data' 'claude'"
        );
    }

    #[test]
    fn attach_nutzt_exaktes_target() {
        assert_eq!(
            cmd_attach("cc-x"),
            "LANG=C.UTF-8 LC_ALL=C.UTF-8 tmux -u attach -t '=cc-x'"
        );
    }

    /// Der `-u`-Schalter ist der eigentliche Fix für kaputte Rahmenzeichen: er erzwingt
    /// UTF-8 im tmux-*Client*, unabhängig davon, ob tmux die Locale korrekt erkennt.
    #[test]
    fn attach_erzwingt_utf8_client() {
        let cmd = cmd_attach("cc-x");
        assert!(cmd.starts_with(LOCALE_PREFIX), "Prefix fehlt: {cmd}");
        assert!(cmd.contains("tmux -u attach"), "-u fehlt: {cmd}");
    }

    /// `C.UTF-8` statt `de_DE.UTF-8`: existiert auf glibc/musl ohne `locale-gen`.
    #[test]
    fn locale_prefix_setzt_lang_und_lc_all() {
        assert_eq!(LOCALE_PREFIX, "LANG=C.UTF-8 LC_ALL=C.UTF-8");
    }

    /// Kill/has-session übertragen keinen Text und bleiben bewusst ohne Prefix.
    #[test]
    fn kill_und_has_session_bleiben_ohne_locale_prefix() {
        assert!(!cmd_kill("cc-x").contains("LC_ALL"));
        assert!(!cmd_has_session("cc-x").contains("LC_ALL"));
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
            "LANG=C.UTF-8 LC_ALL=C.UTF-8 tmux display -p -t '=cc-x' '#{pane_current_path}'"
        );
    }

    #[test]
    fn list_sessions_nutzt_ascii_pipe_und_id_zuerst_name_last() {
        assert_eq!(FIELD_SEP, '|');
        assert_eq!(
            cmd_list_sessions(),
            "LANG=C.UTF-8 LC_ALL=C.UTF-8 tmux list-sessions -F '#{session_id}|#{session_created}|#{session_attached}|#{session_name}' 2>/dev/null || true"
        );
    }

    #[test]
    fn list_panes_nutzt_ascii_pipe_und_id_zuerst_pfad_last() {
        assert_eq!(
            cmd_list_panes(),
            "LANG=C.UTF-8 LC_ALL=C.UTF-8 tmux list-panes -a -F '#{session_id}|#{pane_current_command}|#{pane_current_path}' 2>/dev/null || true"
        );
    }

    #[test]
    fn claude_invocation_ohne_vorgaben_ist_nur_claude() {
        assert_eq!(claude_invocation(None, None), "claude");
    }

    #[test]
    fn claude_invocation_setzt_model_und_effort() {
        assert_eq!(
            claude_invocation(Some("opus"), Some("high")),
            "claude --model 'opus' --effort 'high'"
        );
    }

    #[test]
    fn claude_invocation_laesst_nicht_gesetzte_flags_weg() {
        assert_eq!(claude_invocation(Some("sonnet"), None), "claude --model 'sonnet'");
        assert_eq!(claude_invocation(None, Some("max")), "claude --effort 'max'");
    }

    /// Die Werte stammen aus einer von Hand editierbaren config.json.
    #[test]
    fn claude_invocation_quotet_die_werte() {
        assert_eq!(
            claude_invocation(Some("claude-fable-5; rm -rf /"), None),
            "claude --model 'claude-fable-5; rm -rf /'"
        );
    }

    #[test]
    fn claude_invocation_ignoriert_leere_werte() {
        assert_eq!(claude_invocation(Some(""), Some("  ")), "claude");
    }

    #[test]
    fn scan_projects_joint_mehrere_pfade() {
        assert_eq!(
            cmd_scan_projects(&["/mnt/a".into(), "/mnt/b c".into()]),
            "find '/mnt/a' '/mnt/b c' -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort"
        );
    }
}
