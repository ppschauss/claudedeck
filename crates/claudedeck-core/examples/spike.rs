//! M1-Spike: validiert russh 0.62 end-to-end gegen isekai.local.
//! Modi:
//!   spike <host> <user> exec "<cmd>"     — Kommando ausführen, Output drucken
//!   spike <host> <user> script           — automatisierter PTY-Test (Task 8)
//!   spike <host> <user> attach <name>    — interaktives tmux-Attach (Task 8, echtes TTY)
use russh::client::{self, AuthResult};
use russh::keys::PublicKey;
use russh::ChannelMsg;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, timeout, Duration};

struct SpikeHandler;

impl client::Handler for SpikeHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, _key: &PublicKey) -> Result<bool, Self::Error> {
        Ok(true) // Spike ohne known_hosts-Prüfung — echte Prüfung kommt in M2 (ssh/hostkey.rs)
    }
}

type Handle = client::Handle<SpikeHandler>;

async fn connect(
    host: &str,
    user: &str,
    password: &str,
) -> Result<Handle, Box<dyn std::error::Error>> {
    let config = Arc::new(client::Config::default());
    let mut handle = client::connect(config, (host, 22), SpikeHandler).await?;
    let res = handle.authenticate_password(user, password).await?;
    if !matches!(res, AuthResult::Success) {
        return Err("Authentifizierung fehlgeschlagen".into());
    }
    Ok(handle)
}

async fn exec_capture(
    handle: &Handle,
    cmd: &str,
) -> Result<(String, String, u32), Box<dyn std::error::Error>> {
    let mut channel = handle.channel_open_session().await?;
    channel.exec(true, cmd).await?;
    let (mut out, mut err, mut code) = (Vec::new(), Vec::new(), 0u32);
    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { ref data } => out.extend_from_slice(data),
            ChannelMsg::ExtendedData { ref data, .. } => err.extend_from_slice(data),
            ChannelMsg::ExitStatus { exit_status } => code = exit_status,
            _ => {}
        }
    }
    Ok((
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
        code,
    ))
}

/// Öffnet ein PTY und führt `cmd` darin aus. Gibt den Channel zurück.
async fn open_pty(
    handle: &Handle,
    cmd: &str,
    cols: u32,
    rows: u32,
) -> Result<russh::Channel<client::Msg>, Box<dyn std::error::Error>> {
    let channel = handle.channel_open_session().await?;
    channel
        .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
        .await?;
    channel.exec(true, cmd).await?;
    Ok(channel)
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
async fn script_mode(handle: &Handle) -> Result<(), Box<dyn std::error::Error>> {
    let marker = "SPIKE-OK-1337";
    // Vorab prüfen, ob die Session schon existiert (exit 0 = ja, exit 1 = nein).
    let (_, _, has_session_exit_code) =
        exec_capture(handle, "tmux has-session -t cc-spike").await?;
    let pre_existed = has_session_exit_code == 0;

    // Session idempotent & detached anlegen (Start = exec, Anzeigen = PTY — Spec-Regel)
    exec_capture(handle, "tmux new-session -A -d -s cc-spike").await?;
    let mut channel = open_pty(handle, "tmux attach -t cc-spike", 100, 30).await?;
    let mut writer = channel.make_writer();

    sleep(Duration::from_millis(700)).await; // tmux Zeit zum Zeichnen geben
    writer
        .write_all(format!("echo {marker}\r").as_bytes())
        .await?;
    writer.flush().await?;

    let mut seen = Vec::new();
    let found = timeout(Duration::from_secs(10), async {
        while let Some(msg) = channel.wait().await {
            if let ChannelMsg::Data { ref data } = msg {
                seen.extend_from_slice(data);
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
        exec_capture(handle, "tmux kill-session -t cc-spike")
            .await
            .ok();
    }
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
async fn attach_mode(handle: &Handle, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::terminal;
    use tokio::io::AsyncReadExt;

    let (cols, rows) = terminal::size().unwrap_or((100, 30));
    exec_capture(handle, &format!("tmux new-session -A -d -s {name}")).await?;
    let mut channel = open_pty(
        handle,
        &format!("tmux attach -t {name}"),
        cols as u32,
        rows as u32,
    )
    .await?;
    let mut writer = channel.make_writer();

    terminal::enable_raw_mode()?;
    let result: Result<(), Box<dyn std::error::Error>> = async {
        let mut stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut buf = [0u8; 4096];
        loop {
            tokio::select! {
                msg = channel.wait() => match msg {
                    Some(ChannelMsg::Data { ref data }) => {
                        stdout.write_all(data).await?;
                        stdout.flush().await?;
                    }
                    Some(ChannelMsg::ExitStatus { .. }) | Some(ChannelMsg::Eof) | None => break,
                    _ => {}
                },
                n = stdin.read(&mut buf) => {
                    let n = n?;
                    if n == 0 || buf[..n].contains(&0x1D) { break; } // Strg+] = lokales Detach
                    writer.write_all(&buf[..n]).await?;
                    writer.flush().await?;
                }
            }
        }
        Ok(())
    }
    .await;
    terminal::disable_raw_mode()?;
    println!("\r\n[detached]");
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
    let handle = connect(&host, &user, &password).await?;
    match mode.as_str() {
        "exec" => {
            let cmd = args.get(4).ok_or("exec braucht ein Kommando")?;
            let (out, err, code) = exec_capture(&handle, cmd).await?;
            print!("{out}");
            eprint!("{err}");
            println!("[exit {code}]");
        }
        "script" => script_mode(&handle).await?,
        "attach" => {
            let name = args.get(4).map(String::as_str).unwrap_or("cc-spike");
            attach_mode(&handle, name).await?;
        }
        other => return Err(format!("Unbekannter Modus: {other}").into()),
    }
    Ok(())
}
