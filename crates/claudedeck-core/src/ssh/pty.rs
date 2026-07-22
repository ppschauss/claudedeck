//! PTY-Kanäle: interaktive Sitzungen (tmux attach) mit Streaming-Output.
//!
//! Kanal-Aufteilung: `russh::Channel::split()` liefert `(ChannelReadHalf, ChannelWriteHalf)`.
//! Der Reader läuft exklusiv in einem eigenen `tokio::spawn`-Task (er blockiert in
//! `read_half.wait()`), während `PtyHandle` die `ChannelWriteHalf` behält — `write`, `resize`
//! (`window_change`) und `close` brauchen keinen exklusiven Zugriff auf den Kanal und können
//! daher parallel zum Reader-Task aufgerufen werden, ohne dass sich beide Seiten einen Mutex
//! teilen müssten.

use russh::client;
use russh::{ChannelMsg, ChannelWriteHalf};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use super::connection::ClientHandler;

/// Kanalkapazität für den Output-Stream — wie im validierten Spike bewusst großzügig
/// bemessen, damit PTY-Bursts (z.B. `clear` + Redraw) den SSH-Session-Task nicht blockieren.
const OUTPUT_CHANNEL_CAPACITY: usize = 256;

/// Ereignis aus dem PTY-Reader-Task.
pub enum PtyEvent {
    Data(Vec<u8>),
    Exit(Option<u32>),
}

/// Handle auf eine offene PTY-Sitzung. Der Reader-Task läuft bereits (gestartet in
/// [`PtyHandle::open`]); der Output wird per [`PtyHandle::take_output`] abgeholt.
pub struct PtyHandle {
    write_half: ChannelWriteHalf<client::Msg>,
    output_rx: Option<mpsc::Receiver<PtyEvent>>,
}

impl PtyHandle {
    /// Öffnet einen Session-Kanal, fordert ein PTY an, startet `cmd` darin und spawnt den
    /// Reader-Task. Wird von [`super::connection::SshConnection::open_pty`] genutzt.
    pub(crate) async fn open(
        handle: &client::Handle<ClientHandler>,
        cmd: &str,
        cols: u32,
        rows: u32,
    ) -> Result<Self, russh::Error> {
        let channel = handle.channel_open_session().await?;
        channel
            .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
            .await?;
        channel.exec(true, cmd).await?;

        let (mut read_half, write_half) = channel.split();
        let (tx, rx) = mpsc::channel(OUTPUT_CHANNEL_CAPACITY);

        tokio::spawn(async move {
            let mut exit_code = None;
            while let Some(msg) = read_half.wait().await {
                match msg {
                    ChannelMsg::Data { ref data } => {
                        if tx.send(PtyEvent::Data(data.to_vec())).await.is_err() {
                            return; // Empfänger weg — Task kann sich beenden
                        }
                    }
                    ChannelMsg::ExitStatus { exit_status } => exit_code = Some(exit_status),
                    _ => {}
                }
            }
            let _ = tx.send(PtyEvent::Exit(exit_code)).await;
        });

        Ok(Self {
            write_half,
            output_rx: Some(rx),
        })
    }

    /// Schreibt Bytes auf die Gegenseite (über `make_writer` auf der `ChannelWriteHalf`).
    pub async fn write(&mut self, data: &[u8]) -> Result<(), std::io::Error> {
        let mut writer = self.write_half.make_writer();
        writer.write_all(data).await?;
        writer.flush().await
    }

    /// Teilt der Gegenseite eine neue Terminalgröße mit (`window_change`) — bisher unvalidiert,
    /// wird über den `attach`-Modus im Spike gegen SIGWINCH real getestet (Task 7/Step 2).
    pub async fn resize(&self, cols: u32, rows: u32) -> Result<(), russh::Error> {
        self.write_half.window_change(cols, rows, 0, 0).await
    }

    /// Gibt den Output-Receiver einmalig heraus. Panics bei zweitem Aufruf — der Reader-Task
    /// sendet ohnehin nur an einen einzigen Consumer.
    pub fn take_output(&mut self) -> mpsc::Receiver<PtyEvent> {
        self.output_rx
            .take()
            .expect("PtyHandle::take_output darf nur einmal aufgerufen werden")
    }

    /// Schließt den Kanal. Der Reader-Task beendet sich danach von selbst (wait() -> None).
    pub async fn close(self) -> Result<(), russh::Error> {
        self.write_half.close().await
    }
}
