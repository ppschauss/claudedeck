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

/// Setzt für jedes Kommando, das Text überträgt, eine **tatsächlich vorhandene** UTF-8-Locale.
///
/// Die SSH-Exec-Session kommt ohne Locale-Forwarding (`request_pty` in `ssh/pty.rs` setzt keine
/// Env-Variablen, und sshd lehnt fremde Variablen per `AcceptEnv` üblicherweise ab) — sie läuft
/// also in `C`/`POSIX`. Dort zeichnet tmux keine Rahmenzeichen und Readline verstümmelt
/// 8-Bit-Eingaben, was Umlaute in *beide* Richtungen zerstört.
///
/// **Warum nicht einfach `LC_ALL=C.UTF-8`** (so stand es bis M9 hier): existiert diese Locale
/// auf dem Zielsystem nicht, fällt glibc still auf `ANSI_X3.4-1968` — also ASCII — zurück und
/// gibt bei jedem Kommando eine `setlocale`-Warnung aus. Auf dem Zielserver dieses Projekts ist
/// genau das der Fall (`locale -a` kennt dort nur `C`, `POSIX`, `en_US.utf8`), womit der
/// vermeintliche UTF-8-Fix wirkungslos war.
///
/// Deshalb wird die Locale zur Laufzeit gewählt: die erste, die `locale -a` als UTF-8 meldet.
/// Findet sich keine, bleibt es bei `C` — nicht schlechter als vorher, aber ohne Warnung.
/// Der Aufruf kostet ein `locale -a` pro Kommando; das ist gegenüber dem SSH-Roundtrip nicht
/// messbar.
pub const LOCALE_SETUP: &str = "LC_ALL=\"$(locale -a 2>/dev/null | grep -i -m1 -E 'utf-?8' || echo C)\"; export LC_ALL; LANG=\"$LC_ALL\"; export LANG;";

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
        "{LOCALE_SETUP} tmux list-sessions -F '#{{session_id}}{FIELD_SEP}#{{session_created}}{FIELD_SEP}#{{session_attached}}{FIELD_SEP}#{{session_name}}' 2>/dev/null || true"
    )
}

/// `session_id` zuerst (Matching-Anker statt Namensvergleich), `pane_current_command`
/// (comm-Name, max. 15 Zeichen, praktisch nie mit `|`) in der Mitte, `pane_current_path`
/// (beliebig) LAST.
pub fn cmd_list_panes() -> String {
    format!(
        "{LOCALE_SETUP} tmux list-panes -a -F '#{{session_id}}{FIELD_SEP}#{{pane_current_command}}{FIELD_SEP}#{{pane_current_path}}' 2>/dev/null || true"
    )
}

pub fn cmd_new_detached(name: &str, cwd: &str, command: &str) -> String {
    format!(
        "{LOCALE_SETUP} tmux new-session -A -d -s {} -c {} {}",
        shell_quote(name),
        shell_quote(cwd),
        shell_quote(command)
    )
}

/// `-u` erzwingt UTF-8 im tmux-*Client* — unabhängig davon, ob tmux die Locale erkennt. Ohne das
/// zeichnet Claude Codes TUI keine Rahmenzeichen (siehe [`LOCALE_SETUP`]).
pub fn cmd_attach(name: &str) -> String {
    format!(
        "{LOCALE_SETUP} tmux -u attach -t {}",
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
        "{LOCALE_SETUP} tmux display -p -t {} '#{{pane_current_path}}'",
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

/// Sucht Projektordner unterhalb der `paths`.
///
/// Ein Ordner zählt nur, wenn er eines der `markers` enthält (Vorgabe: `.git`, `.claude`,
/// `CLAUDE.md`). Ohne diesen Filter listet der Scan auf einem Unraid-Server **jedes**
/// Docker-Datenverzeichnis unter `/mnt/cache/appdata` mit — gemessen 88 Einträge statt 9.
/// Eine leere Merkmalsliste bedeutet bewusst „kein Filter" statt „nichts anzeigen".
///
/// Ausgabe je Treffer: `<unix-zeit>\t<pfad>`. Die Zeit ist die **neueste Änderung unter den
/// Einträgen der obersten Ebene**, nicht die des Ordners selbst: letztere ändert sich nur, wenn
/// Dateien hinzukommen oder verschwinden, nicht beim Bearbeiten einer vorhandenen — und genau
/// das will die Sortierung „Zuletzt aktiv" wissen.
///
/// **Kein `sh -c '…'`-Wrapper.** Das Kommando läuft ohnehin durch die Login-Shell des
/// SSH-Servers; ein zusätzlicher Wrapper zerreißt die von [`shell_quote`] gesetzten
/// Anführungszeichen, sobald ein Pfad ein Leerzeichen enthält (Syntaxfehler statt Ergebnis).
/// Aus demselben Grund steht die Locale hier als `export` und nicht als Präfix: eine Zuweisung
/// darf nur vor einem *einfachen* Kommando stehen, nicht vor einer Pipeline oder `{ … }`.
pub fn cmd_scan_projects(paths: &[String], markers: &[String]) -> String {
    let quoted_paths: Vec<String> = paths.iter().map(|p| shell_quote(p)).collect();
    let listing = format!(
        "find {} -mindepth 1 -maxdepth 1 -type d 2>/dev/null",
        quoted_paths.join(" ")
    );
    // Neueste Änderungszeit unter den Einträgen der obersten Ebene, auf ganze Sekunden gekürzt.
    let stamp = "t=$(find \"$d\" -maxdepth 1 -printf '%T@\\n' 2>/dev/null | sort -rn | head -1); printf '%s\\t%s\\n' \"${t%%.*}\" \"$d\"";

    let body = if markers.is_empty() {
        format!("while IFS= read -r d; do {stamp}; done")
    } else {
        let quoted_markers: Vec<String> = markers.iter().map(|m| shell_quote(m)).collect();
        format!(
            "while IFS= read -r d; do for m in {}; do if [ -e \"$d/$m\" ]; then {stamp}; break; fi; done; done",
            quoted_markers.join(" ")
        )
    };

    format!("{LOCALE_SETUP} {listing} | {body} 2>/dev/null || true")
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
            cmd_new_detached("cc-x", "/mnt/cache/app data", "claude")
                .split_once("tmux ")
                .unwrap()
                .1,
            "new-session -A -d -s 'cc-x' -c '/mnt/cache/app data' 'claude'"
        );
    }

    #[test]
    fn attach_nutzt_exaktes_target() {
        assert!(cmd_attach("cc-x").ends_with("tmux -u attach -t '=cc-x'"));
    }

    /// Der `-u`-Schalter ist der eigentliche Fix für kaputte Rahmenzeichen: er erzwingt
    /// UTF-8 im tmux-*Client*, unabhängig davon, ob tmux die Locale korrekt erkennt.
    #[test]
    fn attach_erzwingt_utf8_client() {
        let cmd = cmd_attach("cc-x");
        assert!(cmd.starts_with(LOCALE_SETUP), "Locale-Setup fehlt: {cmd}");
        assert!(cmd.contains("tmux -u attach"), "-u fehlt: {cmd}");
    }

    /// Der eigentliche Test des Locale-Setups: es muss auf einem System mit UTF-8-Locale auch
    /// tatsächlich UTF-8 einstellen.
    ///
    /// Eine reine String-Prüfung („enthält LC_ALL=C.UTF-8") war der Fehler bis M9 — sie blieb
    /// grün, während glibc mangels dieser Locale still auf ASCII zurückfiel.
    #[test]
    fn locale_setup_stellt_auf_utf8_wenn_verfuegbar() {
        let verfuegbar = run("locale -a 2>/dev/null || true").to_lowercase();
        if !verfuegbar.contains("utf") {
            return; // System ohne UTF-8-Locale — dort ist `C` das korrekte Ergebnis.
        }
        let charmap = run(&format!("{LOCALE_SETUP} locale charmap"))
            .trim()
            .to_string();
        assert_eq!(charmap, "UTF-8", "Locale-Setup ergab {charmap} statt UTF-8");
    }

    /// Auf einem System ohne UTF-8-Locale darf nichts Kaputtes gesetzt werden — und vor allem
    /// keine `setlocale`-Warnung entstehen, die jede Ausgabe verschmutzt.
    #[test]
    fn locale_setup_faellt_ohne_utf8_locale_sauber_auf_c_zurueck() {
        // `locale -a` durch eine leere Ausgabe ersetzen simuliert ein System ohne Locales.
        let cmd = LOCALE_SETUP.replace("locale -a 2>/dev/null", "true");
        let gewaehlt = run(&format!("{cmd} printf '%s' \"$LC_ALL\""));
        assert_eq!(gewaehlt, "C");
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
        assert!(cmd_pane_cwd("cc-x").ends_with("tmux display -p -t '=cc-x' '#{pane_current_path}'"));
    }

    #[test]
    fn list_sessions_nutzt_ascii_pipe_und_id_zuerst_name_last() {
        assert_eq!(FIELD_SEP, '|');
        assert_eq!(
            cmd_list_sessions().split_once("tmux ").unwrap().1,
            "list-sessions -F '#{session_id}|#{session_created}|#{session_attached}|#{session_name}' 2>/dev/null || true"
        );
    }

    #[test]
    fn list_panes_nutzt_ascii_pipe_und_id_zuerst_pfad_last() {
        assert_eq!(
            cmd_list_panes().split_once("tmux ").unwrap().1,
            "list-panes -a -F '#{session_id}|#{pane_current_command}|#{pane_current_path}' 2>/dev/null || true"
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
        assert_eq!(
            claude_invocation(Some("sonnet"), None),
            "claude --model 'sonnet'"
        );
        assert_eq!(
            claude_invocation(None, Some("max")),
            "claude --effort 'max'"
        );
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

    /// Führt das erzeugte Kommando gegen ein echtes Verzeichnis aus.
    ///
    /// Der Grund für diesen Aufwand statt einer String-Prüfung: der Vorgänger dieses Kommandos
    /// steckte in einem `sh -c '…'`-Wrapper, in dem die von `shell_quote` erzeugten
    /// Anführungszeichen den Wrapper vorzeitig schlossen. Ein Pfad mit Leerzeichen ergab dadurch
    /// einen Syntaxfehler — während ein `assert!(cmd.contains("'…'"))` fröhlich grün blieb.
    /// Nur Ausführen deckt diese Klasse auf.
    fn run(cmd: &str) -> String {
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .expect("sh muss vorhanden sein");
        assert!(
            out.status.success(),
            "Kommando scheiterte: {}\nstderr: {}",
            cmd,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    #[test]
    fn scan_projects_nimmt_nur_ordner_mit_merkmal() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("echtes-projekt/.git")).unwrap();
        std::fs::create_dir_all(tmp.path().join("docker-daten")).unwrap();

        let out = run(&cmd_scan_projects(
            &[tmp.path().to_string_lossy().into_owned()],
            &[".git".to_string()],
        ));

        assert!(out.contains("echtes-projekt"), "Projekt fehlt: {out}");
        assert!(!out.contains("docker-daten"), "Docker-Ordner drin: {out}");
    }

    /// Genau der Fall, der vorher am Quoting scheiterte.
    #[test]
    fn scan_projects_kommt_mit_leerzeichen_im_pfad_klar() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("mein projekt/.claude")).unwrap();

        let out = run(&cmd_scan_projects(
            &[tmp.path().to_string_lossy().into_owned()],
            &[".claude".to_string()],
        ));

        assert!(
            out.contains("mein projekt"),
            "Pfad mit Leerzeichen fehlt: {out}"
        );
    }

    #[test]
    fn scan_projects_erkennt_jedes_konfigurierte_merkmal() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("mit-git/.git")).unwrap();
        std::fs::create_dir_all(tmp.path().join("mit-claude/.claude")).unwrap();
        std::fs::write(tmp.path().join("mit-md").join("").as_path(), "").ok();
        std::fs::create_dir_all(tmp.path().join("mit-md")).unwrap();
        std::fs::write(tmp.path().join("mit-md/CLAUDE.md"), "x").unwrap();

        let out = run(&cmd_scan_projects(
            &[tmp.path().to_string_lossy().into_owned()],
            &[
                ".git".to_string(),
                ".claude".to_string(),
                "CLAUDE.md".to_string(),
            ],
        ));

        for name in ["mit-git", "mit-claude", "mit-md"] {
            assert!(out.contains(name), "{name} fehlt: {out}");
        }
    }

    /// Jede Zeile trägt einen Unix-Zeitstempel — ohne den fiele die Zeitsortierung in der
    /// Sidebar still auf den Namen zurück, was genau der gemeldete Fehler war.
    #[test]
    fn scan_projects_liefert_zeitstempel_vor_dem_pfad() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("projekt/.git")).unwrap();

        let out = run(&cmd_scan_projects(
            &[tmp.path().to_string_lossy().into_owned()],
            &[".git".to_string()],
        ));

        let line = out.lines().next().expect("eine Zeile erwartet");
        let (stamp, path) = line.split_once('\t').expect("Tab-getrennt erwartet");
        assert!(
            stamp.parse::<i64>().unwrap() > 1_600_000_000,
            "Zeitstempel: {stamp}"
        );
        assert!(path.ends_with("projekt"), "Pfad: {path}");
    }

    #[test]
    fn scan_projects_bleibt_bei_fehlendem_pfad_still() {
        let out = run(&cmd_scan_projects(
            &["/gibt/es/nicht".to_string()],
            &[".git".to_string()],
        ));
        assert_eq!(out.trim(), "");
    }

    /// Kein verschachteltes `sh -c '…'`: das Kommando geht ohnehin durch die Login-Shell des
    /// SSH-Servers, und der Wrapper war die Ursache des Quoting-Fehlers.
    #[test]
    fn scan_kommandos_haben_keinen_verschachtelten_shell_wrapper() {
        assert!(!cmd_scan_projects(&["/mnt/a".into()], &[".git".into()]).contains("sh -c"));
    }
}
