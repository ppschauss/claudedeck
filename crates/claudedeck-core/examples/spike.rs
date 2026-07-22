//! M1-Spike, jetzt gegen die M2-Module (ssh::{connection,exec,pty}) statt gegen einen
//! eingebetteten Handler. Beweist, dass die Refaktorierung das validierte Verhalten erhält —
//! und testet zusätzlich `PtyHandle::resize` (window_change) real gegen SIGWINCH im
//! attach-Modus, was im M1-Spike noch unvalidiert war.
//! Modi:
//!   spike <host> <user> exec "<cmd>"     — Kommando ausführen, Output drucken
//!   spike <host> <user> script           — automatisierter PTY-Test (Task 8)
//!   spike <host> <user> attach <name>    — interaktives tmux-Attach (Task 8, echtes TTY)
use claudedeck_core::ssh::{Auth, ConnectParams, HostkeyPolicy, PtyEvent, SshConnection};
use tokio::time::{sleep, timeout, Duration};

async fn connect(
    host: &str,
    user: &str,
    password: &str,
) -> Result<SshConnection, Box<dyn std::error::Error>> {
    let conn = SshConnection::connect(ConnectParams {
        host: host.to_string(),
        port: 22,
        user: user.to_string(),
        auth: Auth::Password(password.to_string()),
        // Spike prüft bewusst keine known_hosts (echte Prüfung ist ssh::hostkey, per Policy
        // ab Task 8 in der App aktiv) — InsecureAcceptAll + /dev/null spiegelt das alte
        // `check_server_key -> Ok(true)`-Verhalten 1:1.
        known_hosts: std::path::PathBuf::from("/dev/null"),
        policy: HostkeyPolicy::InsecureAcceptAll,
    })
    .await?;
    Ok(conn)
}

/// Automatisierter PTY-Beweis: tmux-Session anlegen, attachen, Marker echoen, Marker im
/// PTY-Output wiederfinden. Exit 0 = PASS.
///
/// Abweichung vom Brief: `tmux has-session` wird VOR dem Anlegen geprüft, damit am Ende nur
/// dann `kill-session` läuft, wenn script_mode die Session selbst frisch erzeugt hat. Der
/// Brief killt cc-spike bedingungslos — das würde Step 4 (Reattach-Semantik gegen eine
/// vorbestehende `watch`-Session) zerstören. Die Verifikations-ERWARTUNG (Session überlebt
/// den script-Lauf, wenn sie vorher schon existierte) ist bindend, dieser Weg dahin ist die
/// Anpassung.
async fn script_mode(conn: &SshConnection) -> Result<(), Box<dyn std::error::Error>> {
    let marker = "SPIKE-OK-1337";
    let has_session = conn.exec_capture("tmux has-session -t cc-spike").await?;
    let pre_existed = has_session.exit_code == Some(0);

    // Session idempotent & detached anlegen (Start = exec, Anzeigen = PTY — Spec-Regel)
    conn.exec_capture("tmux new-session -A -d -s cc-spike")
        .await?;
    let mut pty = conn.open_pty("tmux attach -t cc-spike", 100, 30).await?;
    let mut output = pty.take_output();

    sleep(Duration::from_millis(700)).await; // tmux Zeit zum Zeichnen geben
    pty.write(format!("echo {marker}\r").as_bytes()).await?;

    let mut seen = Vec::new();
    let found = timeout(Duration::from_secs(10), async {
        while let Some(event) = output.recv().await {
            if let PtyEvent::Data(data) = event {
                seen.extend_from_slice(&data);
                // Marker muss 2× auftauchen: einmal als Tipp-Echo, einmal als echter echo-Output
                if String::from_utf8_lossy(&seen).matches(marker).count() >= 2 {
                    return true;
                }
            }
        }
        false
    })
    .await
    .unwrap_or(false);

    // Nur aufräumen, wenn wir die Session selbst frisch angelegt haben — eine vorbestehende
    // Session (z.B. Step 4 mit `watch`) muss den script-Lauf überleben (Kern-Semantik).
    if !pre_existed {
        conn.exec_capture("tmux kill-session -t cc-spike")
            .await
            .ok();
    }
    pty.close().await.ok();

    if found {
        println!("SPIKE PASS — PTY-Streaming über russh funktioniert");
        Ok(())
    } else {
        println!(
            "--- empfangener Output ---\n{}",
            String::from_utf8_lossy(&seen)
        );
        Err("SPIKE FAIL — Marker nicht im PTY-Output gefunden".into())
    }
}

/// Interaktives Attach mit Raw-Terminal. Detach lokal mit Strg+] (0x1D).
/// NEU gegenüber dem M1-Spike: SIGWINCH wird abonniert und löst `PtyHandle::resize`
/// (window_change) aus — der letzte im Brief als "bisher unvalidiert" markierte Pfad.
async fn attach_mode(conn: &SshConnection, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::terminal;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::signal::unix::{signal, SignalKind};

    let (cols, rows) = terminal::size().unwrap_or((100, 30));
    conn.exec_capture(&format!("tmux new-session -A -d -s {name}"))
        .await?;
    let mut pty = conn
        .open_pty(&format!("tmux attach -t {name}"), cols as u32, rows as u32)
        .await?;
    let mut output = pty.take_output();
    let mut winch = signal(SignalKind::window_change())?;

    terminal::enable_raw_mode()?;
    let result: Result<(), Box<dyn std::error::Error>> = async {
        let mut stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut buf = [0u8; 4096];
        loop {
            tokio::select! {
                event = output.recv() => match event {
                    Some(PtyEvent::Data(data)) => {
                        stdout.write_all(&data).await?;
                        stdout.flush().await?;
                    }
                    Some(PtyEvent::Exit(_)) | None => break,
                },
                _ = winch.recv() => {
                    if let Ok((cols, rows)) = terminal::size() {
                        pty.resize(cols as u32, rows as u32).await?;
                    }
                }
                n = stdin.read(&mut buf) => {
                    let n = n?;
                    if n == 0 || buf[..n].contains(&0x1D) { break; } // Strg+] = lokales Detach
                    pty.write(&buf[..n]).await?;
                }
            }
        }
        Ok(())
    }
    .await;
    terminal::disable_raw_mode()?;
    println!("\r\n[detached]");
    pty.close().await.ok();
    result
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let (host, user, mode) = (args.get(1), args.get(2), args.get(3));
    let (host, user, mode) = match (host, user, mode) {
        (Some(h), Some(u), Some(m)) => (h.clone(), u.clone(), m.clone()),
        _ => return Err("Usage: spike <host> <user> exec|script|attach [arg]".into()),
    };
    let password = std::env::var("SPIKE_SSH_PASSWORD").map_err(|_| "SPIKE_SSH_PASSWORD fehlt")?;
    let conn = connect(&host, &user, &password).await?;
    match mode.as_str() {
        "exec" => {
            let cmd = args.get(4).ok_or("exec braucht ein Kommando")?;
            let output = conn.exec_capture(cmd).await?;
            print!("{}", output.stdout);
            eprint!("{}", output.stderr);
            match output.exit_code {
                Some(code) => println!("[exit {code}]"),
                None => println!("[exit ?]"),
            }
        }
        "script" => script_mode(&conn).await?,
        "attach" => {
            let name = args.get(4).map(String::as_str).unwrap_or("cc-spike");
            attach_mode(&conn, name).await?;
        }
        other => return Err(format!("Unbekannter Modus: {other}").into()),
    }
    Ok(())
}
