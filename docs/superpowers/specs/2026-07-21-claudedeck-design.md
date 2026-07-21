# ClaudeDeck — Design

**Datum:** 2026-07-21
**Status:** Freigegeben, bereit für Implementierungsplan

## Zweck

Native Windows-Desktop-App, die sich per SSH mit `isekai.local` verbindet, dort laufende
Claude-Code-Sessions auflistet, sie per Klick als vollwertiges Terminal öffnet und
mehrere davon parallel offen hält. Dateien lassen sich per SFTP hoch- und herunterladen.

Leitplanke: schlank auf beiden Seiten. Weder der Windows-Arbeitsrechner noch die auf
Isekai laufenden Prozesse dürfen spürbar beeinträchtigt werden.

## Rahmenbedingungen

- Ziel-OS: **nur Windows** (10/11).
- Server: Unraid-Host `isekai.local` (LAN `192.168.0.161`, Tailscale `100.87.113.82`),
  tmux unter `/usr/bin/tmux` vorhanden, sshd aktiv.
- `/root` auf Isekai liegt im **RAM** und ist nach Reboot leer. Deshalb: Quellcode unter
  `/mnt/cache/appdata/claudedeck/`, und `~/.claude/projects` ist **keine** verlässliche
  Quelle für die Projektliste.
- Auf dem Unraid-Host gibt es **kein docker-compose-Plugin** (relevant nur für den
  CI-Testcontainer, der in GitHub Actions läuft, nicht auf Isekai).

## Tech-Stack

| Schicht  | Wahl |
|----------|------|
| Shell    | Tauri v2 (nutzt vorhandenes WebView2) |
| Frontend | React + TypeScript + xterm.js (+ fit-addon, search-addon) |
| Backend  | Rust: `russh`, `russh-sftp`, `keyring` |
| Secrets  | Windows Credential Manager via `keyring`-Crate |
| Build    | GitHub Actions (`windows-latest`) → `.msi` + portable `.exe` als Artifact |

**Warum Tauri statt Electron:** ~15 MB Binary und ~100 MB RAM gegenüber ~150 MB Installer
und 250–400 MB RAM. WebView2 ist auf Windows 11 ohnehin geladen. Das Terminal-Rendering ist
bei beiden identisch (xterm.js), es entsteht also kein Qualitätsrisiko — nur etwa ein Tag
Mehraufwand, weil PTY und SFTP in Rust selbst verdrahtet werden müssen.

**Warum GitHub Actions:** Tauri lässt sich von Linux nach Windows nur mit erheblichem
Aufwand cross-kompilieren. Entwickelt wird auf Isekai, gebaut auf `windows-latest`; der
Windows-Rechner braucht keine Toolchain.

## Architektur

### Verbindungsmodell

Eine einzige `russh`-Client-Session zu isekai.local. Darauf laufen mehrere Channels:

- **Pro offener Session ein PTY-Channel**, der `tmux attach -t <name>` bzw. beim Start
  `tmux new-session -A -s cc-<projekt> -c <pfad> claude` ausführt.
- **Ein SFTP-Channel** (`russh-sftp`) für das Dateipanel.
- **Kurzlebige Exec-Channels** für Listenabfragen — nur bei App-Fokus oder expliziter
  Aktion, niemals im Polling-Takt.

Das hält den Server-Footprint konstant: **ein** `sshd`-Prozess und ein TCP-Socket, egal wie
viele Sessions offen sind. Pro offener Session kommt lediglich ein leichter tmux-Client dazu.

Reißt die Verbindung ab, sterben nur die Channels. Die tmux-Sessions und die darin laufenden
Claude-Prozesse laufen unverändert weiter; der Reconnect hängt sich wieder an dieselben an.

### Session-Modell

Die Sidebar führt zwei Quellen in einer Liste zusammen:

1. **Laufende Sessions** aus
   `tmux list-sessions -F '#{session_name}\t#{session_created}\t#{session_attached}'`
   kombiniert mit
   `tmux list-panes -a -F '#{session_name}\t#{pane_current_command}\t#{pane_current_path}'`.
   Sessions, in denen `claude` läuft, werden als Claude-Session markiert; alle anderen
   erscheinen als „Shell" und werden **nicht** ausgeblendet.
2. **Startbare Projekte** aus einem Scan der direkten Unterordner von
   `/mnt/cache/appdata/` (Scanpfade in den Einstellungen konfigurierbar). Ein Klick startet
   `tmux new-session -A -d -s cc-<ordner> -c <pfad> claude` und hängt sich an.

Alles, was die App anlegt, trägt das Präfix `cc-`. So bleibt erkennbar, was von ihr stammt,
und manuell angelegte tmux-Sessions kollidieren nicht. `-A` (attach-if-exists) macht den
Start idempotent.

Session-Namen werden aus Ordnernamen abgeleitet: alles außer `[A-Za-z0-9_-]` wird durch `-`
ersetzt, Länge auf 40 Zeichen begrenzt. Bei Kollision wird `-2`, `-3` … angehängt.

### Sicherheit

- **Host-Key-Prüfung** gegen `%USERPROFILE%\.ssh\known_hosts`. Bei unbekanntem Host ein
  Dialog mit SHA256-Fingerprint; **Abbrechen ist der Default-Button**. Bei geändertem Key
  wird die Verbindung hart abgelehnt, ohne Möglichkeit, im Dialog zu bestätigen.
- **Auth** wahlweise per SSH-Key (Default `%USERPROFILE%\.ssh\id_ed25519`, dann `id_rsa`)
  oder Passwort. Beides wird unterstützt, das Profil legt fest, was benutzt wird.
- **Passwort und Key-Passphrase** landen im Windows Credential Manager (`keyring`), niemals
  im Klartext in der Config.
- Config unter `%APPDATA%\claudedeck\config.json`: Profile (Host, Port, User, Auth-Methode,
  Key-Pfad), Scanpfade, Favoriten, Notification-Einstellungen. Keine Secrets.

## UI

```
┌──────────────┬────────────────────────────────┬──────────┐
│ SESSIONS     │  cc-otakupulse                 │  SFTP    │
│              │                                │          │
│ ● cc-otaku…² │  > claude läuft hier im PTY    │ 📁 src   │
│ ○ cc-habit…  │                                │ 📁 data  │
│ ○ shell-1    │                                │ 📄 .env  │
│ ──────────── │                                │          │
│ + animecut   │                                │ [↑ Drop] │
│ + worktracker│                                │          │
└──────────────┴────────────────────────────────┴──────────┘
  ● angehängt   ○ läuft, nicht offen   + startbar   ² Badge
```

Bewusst **ohne** separate Tab-Leiste: die Sidebar ist der Switcher. Das spart eine komplette
UI-Ebene und eine Quelle für inkonsistenten Zustand.

Das SFTP-Panel ist ein- und ausklappbar (Strg+B).

### Verhalten

- Alle angehängten Sessions bleiben im Hintergrund live. Umschalten ist sofort, es wird
  nichts neu aufgebaut.
- **Badge** zählt Ausgabe-Ereignisse seit dem letzten Ansehen der Session.
- **Windows-Notification** feuert, wenn eine Hintergrund-Session Ausgabe produziert hat und
  danach **>2 s** still ist (Heuristik für „Claude ist fertig / wartet auf Eingabe"). Pro
  Session abschaltbar; ohne diese Bremse wird es bei parallelen Sessions zu Spam.
- **Strg+F** öffnet die Scrollback-Suche. Puffer: 10 000 Zeilen pro Session.
- Fenster- oder Panel-Resize sendet `window-change` an den jeweiligen Channel; tmux passt
  die Session an.
- Die Sidebar aktualisiert sich bei App-Fokus und nach dem Starten oder Beenden einer
  Session — nicht in einem festen Takt.

### SFTP-Panel

Startet im Arbeitsverzeichnis der aktiven Session, ermittelt über
`tmux display -p -t <name> '#{pane_current_path}'`, und folgt ihr beim Umschalten.
Drag&Drop von Windows hinein lädt hoch, Doppelklick auf eine Datei lädt sie in den
Windows-Downloads-Ordner. Überschreiben wird nachgefragt. Transfers laufen über denselben
SFTP-Channel — kein eigener `scp`-Prozess pro Datei.

## Fehlerbehandlung

Jeder Fall bekommt eine sichtbare, konkrete Meldung. Ein totes oder eingefrorenes Terminal
ohne Erklärung ist nie ein akzeptabler Zustand.

| Fall | Verhalten |
|------|-----------|
| Verbindung abgerissen | Overlay „Verbindung verloren – Reconnect in N s", Backoff 3/6/12/30 s, danach manueller Button. Sidebar ausgegraut. |
| Auth fehlgeschlagen | Dialog, **kein** automatischer Retry-Loop (sonst droht eine fail2ban-Sperre). |
| Host-Key unbekannt | Fingerprint-Dialog, Abbruch als Default. |
| Host-Key geändert | Harte Ablehnung mit Warnung, keine Bestätigungsoption in der App. |
| tmux-Session verschwunden | Hinweis im Terminalbereich plus Angebot, sie neu zu starten. |
| tmux fehlt auf dem Server | Klare Meldung mit Installationshinweis statt leerer Liste. |
| SFTP-Fehler (Rechte, kein Platz) | Toast mit Pfad und dem Original-Fehlertext. |
| Scanpfad nicht lesbar | Pfad wird in den Einstellungen als fehlerhaft markiert, App läuft weiter. |

## Tests

- **Rust-Unit:** tmux-Output-Parser (die wahrscheinlichste Bruchstelle), known_hosts-Vergleich
  inklusive Key-Änderung, Session-Namens-Sanitizing und Kollisionsauflösung.
- **Integration in CI:** Service-Container `linuxserver/openssh-server` mit installiertem
  tmux. Getestet werden echtes Attach, Detach, Reconnect an dieselbe Session sowie
  SFTP-Up- und -Download gegen einen echten sshd.
- **Frontend:** vitest für den Session-Store sowie die Badge- und Notification-Logik
  (insbesondere die 2-Sekunden-Stille-Heuristik).
- **Manuelle E2E-Checkliste** für das Rendering: Claudes TUI, Farben, Resize, Maus. Das
  lässt sich nicht sinnvoll automatisieren und wird als Checkliste im Repo geführt.

## Bewusst nicht enthalten (YAGNI)

Split-View, Theme-Editor, Session-Aufzeichnung, Port-Forwarding, Multi-Host-Verwaltung.

Das Profil-Modell ist so angelegt, dass weitere Hosts später ergänzt werden können; v1
verwaltet aber nur isekai.local.
