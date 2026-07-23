/**
 * Verbindungs-Flow, bevor die Hauptoberfläche erscheint (App.tsx rendert entweder dieses oder
 * die Sidebar/TerminalHost/StatusBar-Shell). Ablauf laut Task-5-Auftrag:
 *
 * 1. `hasSecret("password")` → ja: sofort `connect()` versuchen (Keyring-Passwort), ohne
 *    Formular zu zeigen. Nein: Passwort-Formular.
 * 2. `ApiError.kind === "hostkeyUnknown"` → `HostKeyDialog` mit Fingerprint; Bestätigen ruft
 *    `accept_hostkey_and_connect` mit demselben Passwort erneut auf (Keyring-Pfad: `undefined`).
 * 3. `ApiError.kind === "hostkeyChanged"` → reine Fehlanzeige, KEINE Bestätigungsmöglichkeit
 *    (Sicherheitsrisiko/MITM) — nur zurück zum Formular.
 * 4. `ApiError.kind === "authFailed"` (oder jeder andere Fehler) → Meldung + Formular wieder,
 *    kein Auto-Retry (Global Constraint).
 */
import { type FormEvent, useEffect, useRef, useState } from "react";
import { describeApiError, isApiError } from "../lib/apiError";
import { acceptHostkeyAndConnect, connect, hasSecret, saveSecret } from "../lib/ipc";
import { HostKeyDialog } from "./dialogs/HostKeyDialog";

type Phase = "checking" | "form" | "hostkeyUnknown" | "hostkeyChanged";

interface HostkeyInfo {
  fingerprint: string;
  message: string;
}

interface ConnectGateProps {
  onConnected: () => void;
}

export function ConnectGate({ onConnected }: ConnectGateProps) {
  const [phase, setPhase] = useState<Phase>("checking");
  const [password, setPassword] = useState("");
  const [remember, setRemember] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hostkeyInfo, setHostkeyInfo] = useState<HostkeyInfo | null>(null);
  // Passwort des zuletzt versuchten connect() — accept_hostkey_and_connect() braucht denselben
  // Wert (oder `undefined` für den Keyring-Pfad), sonst würde der Nutzer es nach dem
  // Hostkey-Dialog nochmal eintippen müssen.
  const pendingPasswordRef = useRef<string | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const has = await hasSecret("password").catch(() => false);
      if (cancelled) return;
      if (has) {
        await attemptConnect(undefined);
      } else {
        setPhase("form");
      }
    })();
    return () => {
      cancelled = true;
    };
    // Nur beim ersten Mount prüfen — kein Re-Check bei State-Änderungen.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function attemptConnect(pw: string | undefined) {
    setBusy(true);
    setError(null);
    pendingPasswordRef.current = pw;
    try {
      await connect(pw);
      onConnected();
    } catch (err) {
      handleConnectError(err);
    } finally {
      setBusy(false);
    }
  }

  function handleConnectError(err: unknown) {
    if (isApiError(err) && err.kind === "hostkeyUnknown") {
      setHostkeyInfo({ fingerprint: err.fingerprint, message: err.message });
      setPhase("hostkeyUnknown");
      return;
    }
    if (isApiError(err) && err.kind === "hostkeyChanged") {
      setHostkeyInfo({ fingerprint: err.fingerprint, message: err.message });
      setPhase("hostkeyChanged");
      return;
    }
    setError(describeApiError(err));
    setPhase("form");
  }

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      if (remember) {
        await saveSecret("password", password);
      }
    } catch (err) {
      setBusy(false);
      setError(describeApiError(err));
      return;
    }
    setBusy(false);
    await attemptConnect(password);
  }

  async function handleHostkeyConfirm() {
    setBusy(true);
    setError(null);
    try {
      await acceptHostkeyAndConnect(pendingPasswordRef.current);
      onConnected();
    } catch (err) {
      handleConnectError(err);
    } finally {
      setBusy(false);
    }
  }

  function handleHostkeyCancel() {
    setHostkeyInfo(null);
    setPhase("form");
  }

  if (phase === "checking") {
    return (
      <div className="connect-gate">
        <p className="connect-hint">Verbinde…</p>
      </div>
    );
  }

  if (phase === "hostkeyUnknown" && hostkeyInfo) {
    return (
      <div className="connect-gate">
        <HostKeyDialog
          fingerprint={hostkeyInfo.fingerprint}
          busy={busy}
          onCancel={handleHostkeyCancel}
          onConfirm={() => void handleHostkeyConfirm()}
        />
      </div>
    );
  }

  if (phase === "hostkeyChanged" && hostkeyInfo) {
    return (
      <div className="connect-gate">
        <div className="hostkey-changed-box">
          <h2>Host-Key hat sich geändert</h2>
          <p>{hostkeyInfo.message}</p>
          <p className="fingerprint">
            <code>{hostkeyInfo.fingerprint}</code>
          </p>
          <p>
            Das ist ein Sicherheitsrisiko (möglicher Man-in-the-Middle) — diese App verbindet
            sich deshalb NICHT automatisch mit dem neuen Key. Prüfe den Server und ggf. die
            known_hosts-Datei unter <code>%APPDATA%\claudedeck\known_hosts</code> manuell.
          </p>
          <button type="button" onClick={() => setPhase("form")}>
            Zurück
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="connect-gate">
      <form className="connect-form" onSubmit={(e) => void handleSubmit(e)}>
        <h1>ClaudeDeck</h1>
        <label>
          Passwort
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoFocus
            disabled={busy}
          />
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={remember}
            onChange={(e) => setRemember(e.target.checked)}
            disabled={busy}
          />
          Im Windows-Anmeldedaten-Speicher merken
        </label>
        {error && <p className="error-text">{error}</p>}
        <button type="submit" disabled={busy || password.length === 0}>
          Verbinden
        </button>
      </form>
    </div>
  );
}
