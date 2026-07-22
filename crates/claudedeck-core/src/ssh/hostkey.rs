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
        Ok(false) => HostkeyStatus::Unknown {
            fingerprint: fingerprint_sha256(key),
        },
        Err(russh::keys::Error::KeyChanged { .. }) => HostkeyStatus::Changed {
            fingerprint: fingerprint_sha256(key),
        },
        Err(_) => HostkeyStatus::Unknown {
            fingerprint: fingerprint_sha256(key),
        },
    }
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
