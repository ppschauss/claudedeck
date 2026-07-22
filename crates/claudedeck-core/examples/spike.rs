//! M1-Spike: validiert russh 0.62 end-to-end gegen isekai.local.
//! Modi:
//!   spike <host> <user> exec "<cmd>"     — Kommando ausführen, Output drucken
//!   spike <host> <user> script           — automatisierter PTY-Test (Task 8)
//!   spike <host> <user> attach <name>    — interaktives tmux-Attach (Task 8, echtes TTY)
use russh::client::{self, AuthResult};
use russh::keys::PublicKey;
use russh::ChannelMsg;
use std::sync::Arc;

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
        other => return Err(format!("Modus {other} kommt in Task 8").into()),
    }
    Ok(())
}
