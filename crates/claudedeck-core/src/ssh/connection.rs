//! Verbindungsaufbau: Auth (Passwort/Key), Host-Key-Prüfung (Policy-gesteuert), Handle.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use russh::client::{self, AuthResult};
use russh::keys::{PrivateKeyWithHashAlg, PublicKey};

use super::exec::{self, ExecOutput};
use super::hostkey::{self, HostkeyStatus};
use super::pty::PtyHandle;

/// Authentifizierungsverfahren für [`ConnectParams`].
pub enum Auth {
    Password(String),
    Key {
        path: PathBuf,
        passphrase: Option<String>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("Authentifizierung fehlgeschlagen")]
    AuthFailed,
    #[error("Host-Key unbekannt: {fingerprint}")]
    HostkeyUnknown { fingerprint: String },
    #[error("HOST-KEY GEÄNDERT: {fingerprint}")]
    HostkeyChanged { fingerprint: String },
    #[error(transparent)]
    Ssh(#[from] russh::Error),
}

/// Wie mit unbekannten/geänderten Host-Keys umgegangen wird. `Strict` ist die App-Policy,
/// `InsecureAcceptAll` ausschließlich für Tests/Spike gedacht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostkeyPolicy {
    Strict,
    AcceptNew,
    InsecureAcceptAll,
}

pub struct ConnectParams {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: Auth,
    pub known_hosts: PathBuf,
    pub policy: HostkeyPolicy,
}

/// `client::Handler`-Implementierung, die known_hosts + Policy kennt. `remembered` hält den
/// zuletzt abgelehnten Host-Key-Status — russh liefert bei `check_server_key -> Ok(false)` nur
/// einen generischen Fehler, `connect()` übersetzt ihn über diesen Slot in die präzise
/// [`ConnectError`]-Variante.
///
/// `remembered` ist ein `Arc<Mutex<..>>`, kein nackter `Mutex`: der Handler wird von
/// `russh::client::connect` in einen eigenen Tokio-Task verschoben (auch bei Verbindungsfehlern
/// — der Task läuft weiter, bis er sich selbst beendet), `connect()` braucht daher eine eigene
/// Referenz auf denselben Slot, um ihn nach einem gescheiterten `connect`-Aufruf noch auslesen
/// zu können.
pub(crate) struct ClientHandler {
    known_hosts: PathBuf,
    host: String,
    port: u16,
    policy: HostkeyPolicy,
    remembered: Arc<Mutex<Option<HostkeyStatus>>>,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, key: &PublicKey) -> Result<bool, Self::Error> {
        let status = hostkey::check(&self.known_hosts, &self.host, self.port, key);
        let accept = match (&status, self.policy) {
            (HostkeyStatus::Known, _) => true,
            (
                HostkeyStatus::Unknown { .. },
                HostkeyPolicy::AcceptNew | HostkeyPolicy::InsecureAcceptAll,
            ) => {
                hostkey::append(&self.known_hosts, &self.host, self.port, key)?;
                true
            }
            (HostkeyStatus::Changed { .. }, HostkeyPolicy::InsecureAcceptAll) => true,
            (HostkeyStatus::Unknown { .. }, HostkeyPolicy::Strict)
            | (HostkeyStatus::Changed { .. }, _) => false,
        };
        if !accept {
            *self.remembered.lock().unwrap_or_else(|e| e.into_inner()) = Some(status);
        }
        Ok(accept)
    }
}

/// Offene SSH-Verbindung. Kapselt den `russh`-Handle, gegen den `exec_capture`/`open_pty`
/// arbeiten.
pub struct SshConnection {
    handle: client::Handle<ClientHandler>,
}

impl SshConnection {
    pub async fn connect(params: ConnectParams) -> Result<Self, ConnectError> {
        let remembered: Arc<Mutex<Option<HostkeyStatus>>> = Arc::new(Mutex::new(None));
        let handler = ClientHandler {
            known_hosts: params.known_hosts,
            host: params.host.clone(),
            port: params.port,
            policy: params.policy,
            remembered: remembered.clone(),
        };

        let config = Arc::new(client::Config::default());
        let mut handle = client::connect(config, (params.host.as_str(), params.port), handler)
            .await
            .map_err(|err| translate_hostkey_error(&remembered, err))?;

        let auth_result = match params.auth {
            Auth::Password(password) => handle.authenticate_password(params.user, password).await?,
            Auth::Key { path, passphrase } => {
                let key = russh::keys::load_secret_key(&path, passphrase.as_deref())
                    .map_err(russh::Error::from)?;
                let hash_alg = handle.best_supported_rsa_hash().await?.flatten();
                handle
                    .authenticate_publickey(
                        params.user,
                        PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg),
                    )
                    .await?
            }
        };
        if !matches!(auth_result, AuthResult::Success) {
            return Err(ConnectError::AuthFailed);
        }

        Ok(Self { handle })
    }

    pub async fn exec_capture(&self, cmd: &str) -> Result<ExecOutput, russh::Error> {
        exec::capture(&self.handle, cmd).await
    }

    pub async fn open_pty(
        &self,
        cmd: &str,
        cols: u32,
        rows: u32,
    ) -> Result<PtyHandle, russh::Error> {
        PtyHandle::open(&self.handle, cmd, cols, rows).await
    }
}

/// Übersetzt einen generischen `russh`-Fehler aus `client::connect` in die präzise Ursache,
/// falls `check_server_key` einen Host-Key-Status gemerkt hat (Vorrang vor dem generischen
/// Fehler — siehe Doku an [`ClientHandler::remembered`]).
fn translate_hostkey_error(
    remembered: &Arc<Mutex<Option<HostkeyStatus>>>,
    err: russh::Error,
) -> ConnectError {
    let status = remembered.lock().unwrap_or_else(|e| e.into_inner()).take();
    match status {
        Some(HostkeyStatus::Unknown { fingerprint }) => {
            ConnectError::HostkeyUnknown { fingerprint }
        }
        Some(HostkeyStatus::Changed { fingerprint }) => {
            ConnectError::HostkeyChanged { fingerprint }
        }
        _ => ConnectError::Ssh(err),
    }
}
