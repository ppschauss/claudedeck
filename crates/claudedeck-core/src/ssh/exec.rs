//! Kommando-Ausführung ohne PTY (`exec`-Kanal, kein Terminal auf der Gegenseite).

use russh::client;
use russh::ChannelMsg;

use super::connection::ClientHandler;

/// Ergebnis eines nicht-interaktiven Kommandos.
#[derive(Debug, Clone)]
pub struct ExecOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<u32>,
}

impl ExecOutput {
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }
}

/// Öffnet einen Session-Kanal, führt `cmd` aus und sammelt stdout/stderr/Exit-Code bis der
/// Kanal geschlossen wird. Wird von [`super::connection::SshConnection::exec_capture`] genutzt.
pub(crate) async fn capture(
    handle: &client::Handle<ClientHandler>,
    cmd: &str,
) -> Result<ExecOutput, russh::Error> {
    let mut channel = handle.channel_open_session().await?;
    channel.exec(true, cmd).await?;

    let (mut out, mut err, mut exit_code) = (Vec::new(), Vec::new(), None);
    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { ref data } => out.extend_from_slice(data),
            ChannelMsg::ExtendedData { ref data, .. } => err.extend_from_slice(data),
            ChannelMsg::ExitStatus { exit_status } => exit_code = Some(exit_status),
            _ => {}
        }
    }

    Ok(ExecOutput {
        stdout: String::from_utf8_lossy(&out).into_owned(),
        stderr: String::from_utf8_lossy(&err).into_owned(),
        exit_code,
    })
}
