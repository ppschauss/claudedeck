//! `ApiError`: einziger Fehlertyp, den Tauri-Commands nach außen geben. Serialisiert sich
//! selbst in die im IPC-Contract festgelegte Form
//! `{ kind: "authFailed"|"hostkeyUnknown"|"hostkeyChanged"|"notConnected"|"tmuxMissing"|"ssh"|"io", message: string, fingerprint?: string }`
//! — das Frontend schaltet auf `kind`, insbesondere für den Hostkey-Dialog bei
//! `hostkeyUnknown`.
//!
//! `notConnected`/`tmuxMissing` werden erst von Task 3 (Session-Commands) erzeugt, sind aber
//! hier schon angelegt, damit der Contract von Anfang an vollständig ist.

use claudedeck_core::ssh::ConnectError;
use serde::Serialize;

#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
// NotConnected/TmuxMissing werden erst von Task 3s Session-Commands konstruiert — die Variante
// existiert bereits jetzt, damit der IPC-Contract von Anfang an vollständig ist; ansonsten
// meldet `-D warnings` "never constructed".
#[allow(dead_code)]
pub enum ApiError {
    #[error("{message}")]
    AuthFailed { message: String },
    #[error("{message}")]
    HostkeyUnknown { message: String, fingerprint: String },
    #[error("{message}")]
    HostkeyChanged { message: String, fingerprint: String },
    #[error("{message}")]
    NotConnected { message: String },
    #[error("{message}")]
    TmuxMissing { message: String },
    #[error("{message}")]
    Ssh { message: String },
    #[error("{message}")]
    Io { message: String },
}

/// Übersetzt `claudedeck_core::ssh::ConnectError` 1:1 in die passende `ApiError`-Variante.
/// `message` wird einmal vor dem `match` aus `Display` gebildet (die `#[error(...)]`-Texte in
/// `ConnectError` selbst) — für `Ssh(_)` ist das exakt der `russh::Error`-Text, weil
/// `ConnectError::Ssh` `#[error(transparent)]` ist.
impl From<ConnectError> for ApiError {
    fn from(err: ConnectError) -> Self {
        let message = err.to_string();
        match err {
            ConnectError::AuthFailed => ApiError::AuthFailed { message },
            ConnectError::HostkeyUnknown { fingerprint } => {
                ApiError::HostkeyUnknown { message, fingerprint }
            }
            ConnectError::HostkeyChanged { fingerprint } => {
                ApiError::HostkeyChanged { message, fingerprint }
            }
            ConnectError::Ssh(_) => ApiError::Ssh { message },
        }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        ApiError::Io { message: err.to_string() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostkey_unknown_serialisiert_kind_message_und_fingerprint() {
        let err = ApiError::HostkeyUnknown {
            message: "Host-Key unbekannt: SHA256:abc".to_string(),
            fingerprint: "SHA256:abc".to_string(),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "hostkeyUnknown",
                "message": "Host-Key unbekannt: SHA256:abc",
                "fingerprint": "SHA256:abc",
            })
        );
    }

    #[test]
    fn not_connected_serialisiert_ohne_fingerprint_feld() {
        let err = ApiError::NotConnected { message: "nicht verbunden".to_string() };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "kind": "notConnected", "message": "nicht verbunden" })
        );
        assert!(json.get("fingerprint").is_none());
    }

    #[test]
    fn tmux_missing_kind_ist_camel_case() {
        let err = ApiError::TmuxMissing { message: "tmux fehlt".to_string() };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["kind"], "tmuxMissing");
    }

    #[test]
    fn from_connect_error_auth_failed() {
        let api: ApiError = ConnectError::AuthFailed.into();
        match api {
            ApiError::AuthFailed { message } => assert!(!message.is_empty()),
            other => panic!("erwartet AuthFailed, war {other:?}"),
        }
    }

    #[test]
    fn from_connect_error_hostkey_unknown_traegt_fingerprint() {
        let api: ApiError = ConnectError::HostkeyUnknown { fingerprint: "SHA256:xyz".to_string() }.into();
        match api {
            ApiError::HostkeyUnknown { fingerprint, .. } => assert_eq!(fingerprint, "SHA256:xyz"),
            other => panic!("erwartet HostkeyUnknown, war {other:?}"),
        }
    }
}
