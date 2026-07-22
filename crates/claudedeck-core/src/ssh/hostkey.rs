use russh::keys::PublicKey;
use std::io::Write;
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub enum HostkeyStatus {
    Known,
    Unknown { fingerprint: String },
    Changed { fingerprint: String },
}

pub fn fingerprint_sha256(key: &PublicKey) -> String {
    key.fingerprint(Default::default()).to_string()
}

pub fn check(known_hosts: &Path, host: &str, port: u16, key: &PublicKey) -> HostkeyStatus {
    // ssh_key::PublicKey::eq() compares the comment field too, but keys recorded
    // in known_hosts never carry one (russh::keys::parse_public_key_base64 parses
    // only the raw key blob). Strip the comment before comparing so a key with a
    // comment (e.g. from PublicKey::from_openssh) still matches its known_hosts entry.
    let query_key = PublicKey::from(key.key_data().clone());
    match russh::keys::check_known_hosts_path(host, port, &query_key, known_hosts) {
        Ok(true) => HostkeyStatus::Known,
        // russh only returns Err(KeyChanged) when it finds an entry for this host with the
        // SAME key type but a DIFFERENT key. If the server now presents a different key TYPE
        // (e.g. RSA where known_hosts has ed25519), no entry of that type matches, so russh
        // reports Ok(false) — indistinguishable, on its own, from a genuinely new host. Under
        // HostkeyPolicy::AcceptNew that would silently accept a MITM using a different
        // algorithm. So on Ok(false) we independently check known_hosts for ANY entry for this
        // host (any key type) — if one exists, treat it as Changed, not Unknown.
        Ok(false) => {
            if host_has_any_entry(host, port, known_hosts) {
                HostkeyStatus::Changed {
                    fingerprint: fingerprint_sha256(key),
                }
            } else {
                HostkeyStatus::Unknown {
                    fingerprint: fingerprint_sha256(key),
                }
            }
        }
        Err(russh::keys::Error::KeyChanged { .. }) => HostkeyStatus::Changed {
            fingerprint: fingerprint_sha256(key),
        },
        Err(_) => HostkeyStatus::Unknown {
            fingerprint: fingerprint_sha256(key),
        },
    }
}

/// Prüft, ob `known_hosts` für `host`/`port` IRGENDEINEN Eintrag enthält — unabhängig vom
/// Key-Typ. Wird genutzt, um einen Algorithmus-Wechsel (anderer Key-Typ als hinterlegt) von
/// einem echten neuen Host zu unterscheiden (siehe `check`).
///
/// Delegiert an russhs eigene, bereits getestete `known_host_keys_path`-Matching-Logik (führendes
/// Whitespace, Kommentarzeilen, komma-separierte Hostlisten, `[host]:port`-Klammernotation UND
/// gehashte Einträge `|1|<salt>|<hash> ...` — die per HMAC-SHA1 gegen `host:port` aufgelöst
/// werden, siehe `russh::keys::known_hosts::match_hostname`). Damit werden auch gehashte
/// Einträge (Debian/Ubuntu-Default `HashKnownHosts yes`) korrekt erkannt.
fn host_has_any_entry(host: &str, port: u16, known_hosts: &Path) -> bool {
    russh::keys::known_hosts::known_host_keys_path(host, port, known_hosts)
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

pub fn append(known_hosts: &Path, host: &str, port: u16, key: &PublicKey) -> std::io::Result<()> {
    if let Some(dir) = known_hosts.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let host_field = if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    };
    let key_str = key.to_openssh().map_err(std::io::Error::other)?;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(known_hosts)?;
    writeln!(f, "{host_field} {key_str}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // Zwei echte, feste Ed25519-Testschlüssel (nur Testdaten, keine Secrets):
    const KEY_A: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIGb0eNSXSGcE8YG5RuRhZs2NM4Z2zAtxKT9d6sPCLsdE";
    const KEY_B: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIODJol6WSDGaX8DJHfF9O5B84vLdU21LMc0dGE0hMh8I";
    // Echter RSA-2048-Testschlüssel (nur Testdaten, keine Secrets), generiert via
    // `ssh-keygen -t rsa -b 2048` im Dev-Container — dient nur zum Nachweis, dass ein
    // Algorithmus-Wechsel (RSA in known_hosts, Ed25519 in der Query) als Changed erkannt wird.
    const RSA_KEY_A: &str = "AAAAB3NzaC1yc2EAAAADAQABAAABAQC3EGcgKqU4V5wzLFlku5W2cwieVleY5QjzKeGsw5aGRONqx5AMLPc0HwiFNio21Fol8ZRT9wJQMqDDwEIlh55qyMzp1II8NY9BxS8J8pvmAfj71FGQdhPpQUhU5GJ9N9c1pFWUfeJF7MVm5ZeDBe6hwY7N+ABH3EgagwUvxuY1RrLlNKT4yIRcqGQNJbcKjZzXTIgCa/mfzdDUCVwFvmtKK34WfvTbPAksgoCXEqR25lhzG8Tf2QRvb11XQc2S/e8tS5ztAT9F4R3I3Uf+2Ps3GsgbsDSEqCrXRBSjt3WyWUZSuXuAywF6uVz+Ci1vYa2kkjp2oJpMnItNZklWpV17";

    fn pk(b64: &str) -> russh::keys::PublicKey {
        russh::keys::PublicKey::from_openssh(&format!("ssh-ed25519 {b64} test")).unwrap()
    }

    fn kh(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn unbekannter_host_liefert_unknown_mit_fingerprint() {
        let f = kh("");
        let st = check(f.path(), "isekai.local", 22, &pk(KEY_A));
        match st {
            HostkeyStatus::Unknown { fingerprint } => assert!(fingerprint.starts_with("SHA256:")),
            other => panic!("erwartet Unknown, war {other:?}"),
        }
    }

    #[test]
    fn bekannter_host_liefert_known() {
        let f = kh(&format!("isekai.local ssh-ed25519 {KEY_A}\n"));
        assert_eq!(
            check(f.path(), "isekai.local", 22, &pk(KEY_A)),
            HostkeyStatus::Known
        );
    }

    #[test]
    fn geaenderter_key_liefert_changed() {
        let f = kh(&format!("isekai.local ssh-ed25519 {KEY_A}\n"));
        match check(f.path(), "isekai.local", 22, &pk(KEY_B)) {
            HostkeyStatus::Changed { .. } => {}
            other => panic!("erwartet Changed, war {other:?}"),
        }
    }

    #[test]
    fn nichtstandard_port_nutzt_klammer_notation() {
        let f = kh(&format!("[isekai.local]:2222 ssh-ed25519 {KEY_A}\n"));
        assert_eq!(
            check(f.path(), "isekai.local", 2222, &pk(KEY_A)),
            HostkeyStatus::Known
        );
    }

    #[test]
    fn append_schreibt_und_check_findet() {
        let f = kh("");
        append(f.path(), "neu.local", 2222, &pk(KEY_A)).unwrap();
        assert_eq!(
            check(f.path(), "neu.local", 2222, &pk(KEY_A)),
            HostkeyStatus::Known
        );
        let content = std::fs::read_to_string(f.path()).unwrap();
        assert!(content.contains("[neu.local]:2222 ssh-ed25519 "));
    }

    #[test]
    fn anderer_key_typ_ist_changed() {
        // known_hosts hat für isekai.local einen RSA-Eintrag hinterlegt; die Query kommt mit
        // einem Ed25519-Key (KEY_A) für denselben Host. russh's check_known_hosts_path liefert
        // hier Ok(false) (kein Eintrag *dieses* Typs matcht), was ohne den Fix fälschlich als
        // Unknown durchgeht — unter AcceptNew würde ein Algorithmus-Wechsel-MITM so unbemerkt
        // als "neuer Host" akzeptiert. Da known_hosts aber sehr wohl einen Eintrag für den Host
        // hat (nur mit anderem Key-Typ), muss das Ergebnis Changed sein.
        let f = kh(&format!("isekai.local ssh-rsa {RSA_KEY_A}\n"));
        match check(f.path(), "isekai.local", 22, &pk(KEY_A)) {
            HostkeyStatus::Changed { .. } => {}
            other => panic!("erwartet Changed, war {other:?}"),
        }
    }

    #[test]
    fn gehashter_eintrag_mit_anderem_key_typ_ist_changed() {
        // known_hosts hat für isekai.local einen GEHASHTEN (`|1|<salt>|<hash>`, wie es
        // Debian/Ubuntu per HashKnownHosts-Default erzeugt) RSA-Eintrag hinterlegt; die Query
        // kommt mit dem Ed25519-Key KEY_A für denselben Host. Die gehashte Zeile ist deterministisch
        // vorgeneriert (kein Laufzeit-ssh-keygen im Test) via:
        //   `ssh-keygen -H -f <datei mit Klartextzeile "isekai.local ssh-rsa {RSA_KEY_A}">`
        // Erwartung: wie beim Klartext-Fall (anderer_key_typ_ist_changed) muss das Ergebnis
        // Changed sein, nicht Unknown — sonst bliebe ein Algorithmus-Wechsel-MITM unter
        // HostkeyPolicy::AcceptNew unerkannt, sobald known_hosts gehasht ist.
        let f = kh(&format!(
            "|1|AXF7D6hwxHqIWTtHK3yPGyId6Vs=|AQU9Xh+vduAeEJmFHvUIfXJCUjw= ssh-rsa {RSA_KEY_A}\n"
        ));
        match check(f.path(), "isekai.local", 22, &pk(KEY_A)) {
            HostkeyStatus::Changed { .. } => {}
            other => panic!("erwartet Changed, war {other:?}"),
        }
    }

    #[test]
    fn fehlende_datei_ist_unknown_nicht_panic() {
        let st = check(
            std::path::Path::new("/nonexistent/known_hosts"),
            "x",
            22,
            &pk(KEY_A),
        );
        assert!(matches!(st, HostkeyStatus::Unknown { .. }));
    }
}
