//! Integrationstests gegen einen echten sshd. Lokal:
//!   CLAUDEDECK_TEST_SSH=192.168.0.161:22:root:$SPIKE_SSH_PASSWORD ./dev.sh cargo test -p claudedeck-core --test integration_ssh -- --ignored
//! In CI: Service-Container (siehe ci.yml).
use claudedeck_core::ssh::connection::{Auth, ConnectParams, HostkeyPolicy, SshConnection};
use claudedeck_core::tmux::{commands, parser};

fn params() -> ConnectParams {
    let raw = std::env::var("CLAUDEDECK_TEST_SSH").expect("CLAUDEDECK_TEST_SSH fehlt");
    let p: Vec<&str> = raw.splitn(4, ':').collect();
    ConnectParams {
        host: p[0].into(),
        port: p[1].parse().unwrap(),
        user: p[2].into(),
        auth: Auth::Password(p[3].into()),
        known_hosts: std::path::PathBuf::from("/dev/null"),
        policy: HostkeyPolicy::InsecureAcceptAll,
    }
}

const S: &str = "cc-inttest";

async fn cleanup(conn: &SshConnection) {
    let _ = conn.exec_capture(&commands::cmd_kill(S)).await;
}

/// cargo test führt Testfunktionen standardmäßig parallel in Threads desselben Prozesses aus.
/// Die drei Tests unten teilen sich denselben Remote-Sessionnamen `S` — ohne Serialisierung
/// räumt ein Test die Session eines anderen währenddessen weg (beobachtet: verschwundene
/// Session, leerer `window_width`-Query). Ein prozessweiter Mutex serialisiert nur diese drei;
/// `exec_liefert_stdout_und_exitcode` und der Auth-Test rühren `S` nicht an und bleiben parallel.
fn session_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[tokio::test]
#[ignore]
async fn exec_liefert_stdout_und_exitcode() {
    let conn = SshConnection::connect(params()).await.unwrap();
    let out = conn.exec_capture("echo hallo && exit 3").await.unwrap();
    assert_eq!(out.stdout.trim(), "hallo");
    assert_eq!(out.exit_code, Some(3));
}

#[tokio::test]
#[ignore]
async fn tmux_roundtrip_liste_und_parser() {
    let _guard = session_lock().lock().await;
    let conn = SshConnection::connect(params()).await.unwrap();
    cleanup(&conn).await;
    conn.exec_capture(&commands::cmd_new_detached(S, "/tmp", "sh"))
        .await
        .unwrap();
    let ls = conn
        .exec_capture(&commands::cmd_list_sessions())
        .await
        .unwrap();
    let sessions = parser::parse_sessions(&ls.stdout);
    assert!(
        sessions.iter().any(|s| s.name == S),
        "Session fehlt in: {}",
        ls.stdout
    );
    cleanup(&conn).await;
}

#[tokio::test]
#[ignore]
async fn pty_attach_marker_und_reattach_semantik() {
    let _guard = session_lock().lock().await;
    let conn = SshConnection::connect(params()).await.unwrap();
    cleanup(&conn).await;
    conn.exec_capture(&commands::cmd_new_detached(S, "/tmp", "sh"))
        .await
        .unwrap();

    // Attach 1: Marker tippen
    let mut pty = conn
        .open_pty(&commands::cmd_attach(S), 100, 30)
        .await
        .unwrap();
    let mut rx = pty.take_output();
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    pty.write(b"echo INT-MARKER-1\r").await.unwrap();
    let mut seen = Vec::new();
    let ok = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while let Some(ev) = rx.recv().await {
            if let claudedeck_core::ssh::pty::PtyEvent::Data(d) = ev {
                seen.extend_from_slice(&d);
                if String::from_utf8_lossy(&seen)
                    .matches("INT-MARKER-1")
                    .count()
                    >= 2
                {
                    return true;
                }
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(
        ok,
        "Marker nicht gesehen: {}",
        String::from_utf8_lossy(&seen)
    );
    pty.close().await.unwrap();

    // Session lebt nach Channel-Close weiter (Kern-Semantik)
    let has = conn
        .exec_capture(&commands::cmd_has_session(S))
        .await
        .unwrap();
    assert_eq!(has.exit_code, Some(0), "Session starb mit dem Channel!");

    // Attach 2: Marker steht im Scrollback/Screen
    let mut pty2 = conn
        .open_pty(&commands::cmd_attach(S), 100, 30)
        .await
        .unwrap();
    let mut rx2 = pty2.take_output();
    let mut seen2 = Vec::new();
    let ok2 = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while let Some(ev) = rx2.recv().await {
            if let claudedeck_core::ssh::pty::PtyEvent::Data(d) = ev {
                seen2.extend_from_slice(&d);
                if String::from_utf8_lossy(&seen2).contains("INT-MARKER-1") {
                    return true;
                }
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(ok2, "Marker nach Reattach nicht sichtbar");
    cleanup(&conn).await;
}

#[tokio::test]
#[ignore]
async fn resize_aendert_tmux_fensterbreite() {
    let _guard = session_lock().lock().await;
    let conn = SshConnection::connect(params()).await.unwrap();
    cleanup(&conn).await;
    conn.exec_capture(&commands::cmd_new_detached(S, "/tmp", "sh"))
        .await
        .unwrap();
    let pty = conn
        .open_pty(&commands::cmd_attach(S), 80, 24)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    pty.resize(123, 40).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    // `-t '=cc-inttest'` (Session ohne Fenster-Qualifier) liefert #{window_width} leer zurück
    // (verifiziert gegen echten tmux 3.3a und 3.5a) — mit explizitem Fensterindex ':0' liefert
    // dieselbe Exakt-Match-Syntax den erwarteten Wert.
    let w = conn
        .exec_capture(&format!(
            "tmux display -p -t {} '#{{window_width}}'",
            commands::shell_quote(&format!("={S}:0"))
        ))
        .await
        .unwrap();
    assert_eq!(w.stdout.trim(), "123");
    cleanup(&conn).await;
}

#[tokio::test]
#[ignore]
async fn falsches_passwort_ist_authfailed_ohne_retry() {
    let mut p = params();
    p.auth = Auth::Password("definitiv-falsch".into());
    match SshConnection::connect(p).await {
        Err(claudedeck_core::ssh::connection::ConnectError::AuthFailed) => {}
        Err(other) => panic!("erwartet AuthFailed, war {other:?}"),
        Ok(_) => panic!("erwartet AuthFailed, aber Verbindung erfolgreich"),
    }
}
