# Ideen-Halde: Plane-Anbindung & Projekt-Automatisierung

Stand 2026-11-04. Sortiert nach „blockiert den Loop" / „danach" / „nicht bauen".
Der Loop = Plane-Task auf „Todo" → Dispatcher startet tmux-Session mit Claude im
Projektpfad → ClaudeDeck zeigt sie. Alles andere ist Komfort.

## Architektur: zweigeteilt, Client speichert nichts

Entschieden: Backend auf dem Server haelt alle Daten, das Programm ist Schluessel +
visuelle Oberflaeche. Uebertragung verschluesselt.

**Transport bleibt SSH**, keine eigene API. Begruendung:
- Verschluesselung, Auth und Host-Key-Verifikation sind vorhanden und erprobt
  (`ssh/connection.rs`, `ssh/hostkey.rs`). Eine eigene TLS-API braeuchte all das neu.
- Kein offener Port ausser 22, keine Zertifikatsverwaltung, kein selbstgebautes
  Token-System — letzteres waere die riskanteste Komponente im Konstrukt.
- SSH-Key-Auth *ist* das Konzept „das Programm ist der Schluessel". Key auf Stick
  oder YubiKey, dann ist das Geraet tatsaechlich leer.
- Eine HTTPS/WebSocket-API lohnt erst fuer Browser- oder Mobile-Clients. Additiv,
  nicht jetzt.

**Server** — `crates/claudedeck-backend`, neues Crate:
- Config, Profile, Projekt-Metadaten, Secrets-*Referenzen* (Ablage unter
  `/var/lib/claudedeck/`)
- Dispatcher (Plane-Webhooks)
- antwortet auf JSON-Kommandos ueber den SSH-Kanal, statt dass der Client
  `tmux list-sessions` parst

**Client** — ClaudeDeck:
- Login, Terminal-Rendering, UI
- `get_config` holt vom Server statt von Platte
- `keyring`-Dependency raus, `config_path()`/`save_to()` raus

**Umbaugroesse**: `config::load_from(&config::config_path())` an 5 Stellen, alle in
`src-tauri/src/commands/`. Das `Config`-Struct selbst ist transport-frei und bleibt
unveraendert — Transport-Tausch, kein Rewrite.

**Was zwangsläufig lokal bleibt** (keine Projektdaten, keine Secrets):
1. Host/Port des Servers — ohne das kein Start.
2. Der SSH-Private-Key bei Key-Auth. Auf externem Medium haltbar.
3. Der Host-Key-Fingerprint. Bewusst gespeichert: die Alternative waere Bestaetigen
   bei jedem Start, was zu Blind-Wegklicken fuehrt und real schlechter gegen MITM
   schuetzt als eine gespeicherte Zeile.

**Offene Annahme**: „sofern da keine Arbeitsumgebung hinterlegt ist" = zwei Modi,
Remote (nichts lokal ausser den drei Punkten) und Lokal (Projekt liegt auf dem Geraet,
dortige Ablage erlaubt). Entscheidet, ob `config.rs` zwei Backends braucht oder eines.


## Blockiert den Loop

### Projekt-Anlage als Script
`new-project.sh <slug>` legt an:
- `/srv/projects/<slug>/` + `git init`
- `CLAUDE.md` aus Template (siehe unten)
- `SECRETS.md` aus Template (Referenzen, keine Werte)
- langlebige Projekt-Session: `tmux new-session -A -d -s proj-<slug> -c <pfad> claude`

Kein App-Feature. Shell + zwei Textdateien.

### Worktree-Entscheidung
Problem: „Session bleibt bestehen" + „neue Aufgabe = neue Session im selben Pfad"
= mehrere Claude-Instanzen im selben Working-Tree, die übereinander branchen/committen.

Entschieden: Projekt-Session in der Wurzel (committet nie, dient Fragen/Doku/Überblick),
Aufgaben-Sessions je in eigenem Worktree:

    git -C <projekt> worktree add .worktrees/<ISSUE> -b task/<ISSUE>
    tmux new-session -A -d -s task-<ISSUE> -c <projekt>/.worktrees/<ISSUE> claude

`.worktrees/` in `.gitignore`. Aufräumen nach Merge: `git worktree remove`.
Betrifft den Dispatcher direkt — der setzt den Pfad.

### Dispatcher
`crates/claudedeck-dispatch`, läuft als systemd-Unit auf dem Homelab.
- Plane-Webhook empfangen, Signatur prüfen (Schema versionsabhängig: älter
  `X-Plane-Signature` HMAC-SHA256, neuer via Svix — in der eigenen Instanz verifizieren)
- Projekt-Whitelist, keine freien Pfade aus dem Payload
- State-UUIDs beim Start über die API auflösen und cachen (Webhooks liefern UUIDs,
  nicht Namen; „Todo" ist pro Projekt eine andere UUID)
- Idempotent durch `tmux new-session -A` + deterministischen Namen `task-<ISSUE>`
  (Plane retryt bei non-200)
- nutzt `claudedeck-core::tmux::commands` als Lib — nicht in `src-tauri`, die App ist
  Windows-Client, der Dispatcher ist Server-Daemon

### CLAUDE.md-Template (Projekt)
Inhalt sinngemäß:
- Arbeite ausschließlich in diesem Pfad.
- Führe alle verwendeten Frameworks/Tools/Versionen hier auf, bei Änderung nachtragen.
- Holst du etwas aus einem anderen Projekt: Kopie hierher, Herkunft in der Historie
  vermerken, **und die Kopie committen** — im Worktree ist sie sonst beim Merge weg.
- Historie als `## Historie`-Abschnitt, neueste Einträge oben, mit Datum.
- Secrets: siehe `SECRETS.md`. Werte niemals in Dateien, Logs oder Historie.

### Key-Auth (vom Nice-to-have zum Kernfeature)
Offener Punkt aus dem README, durch die Client-Architektur jetzt zentral: Key-Auth ist
der Mechanismus, mit dem das Programm „der Schluessel" ist. `AuthMethod` existiert in
`config.rs`, `connection.rs:98` liest `key_path` schon aus der Config.


## Danach, als Plane-Issue

- **`@plane_issue` durchziehen** (M10): tmux user-option beim Start setzen, in
  `cmd_list_sessions` mitlesen, über `parser.rs`/`ipc.ts` in die Sidebar. Issue-ID ist
  separatorfrei und darf vor den Namen; der **Titel** nie in `list-sessions` (enthält
  irgendwann ein `|`), der kommt aus der Plane-API.
- **Sidebar-Gruppe „Aufgaben"** als vierte Gruppe. `SidebarGroup` existiert.
- **Rückkanal** (M12): Session endet → Issue auf „Done", PR-Link als Kommentar.
- **Claude-Funktionen visuell**: `catalog/` scannt Slash-Commands und MCP-Connectors
  bereits. Was darüber hinaus wirklich fehlt, zeigt sich erst nach ein paar Wochen
  Benutzen — bis dahin nicht spekulieren.
- **Zentrale Wissensbasis** `/srv/knowledge/` (standards/, clients/, decisions/),
  global per `@`-Import in `~/.claude/CLAUDE.md`, projektspezifisch per Import im
  Projekt-`CLAUDE.md`. Ab ~20 Dateien auf einen MCP-Server über das Verzeichnis
  umstellen: Imports landen in *jedem* Prompt und fluten den Kontext.

## Nicht bauen

- **PM-Features** (Projekte, Terminierung, Kommentare, Zuweisung, Team, Notifications,
  Milestones). Das ist vollständig Plane; Milestones = Cycles/Modules. Nachbauen heißt
  Plane in Rust neu schreiben und ist der einzige Weg, dieses Projekt zum Scheitern zu
  bringen. ClaudeDeck = Terminal-Monitor. Plane = PM. Dispatcher = Klebstoff.
- ~~**Vaultwarden als Ersatz fuer den Windows-Credential-Store.**~~ **Revidiert
  2026-11-04**: Mit „Client speichert nichts" ist die lokale DPAPI-Speicherung der
  Gegenentwurf, nicht die Loesung. Wird ein Rueckbau — `keyring` raus, Profile und
  Secrets-Referenzen wandern auf den Server. Die `NamedProfile`/`AuthMethod`-Modelle
  bleiben nutzbar, nur ihre Ablage wechselt.
- **Vaultwarden für den unattended Dispatcher.** API-Key-Login lässt das Vault gesperrt,
  es braucht ein `BW_SESSION`, und ein zweites `bw unlock` invalidiert die erste Session.
  Dauerhaft entsperrtes Vault auf einem Daemon = wer den Server hat, hat alle Secrets.
  Machine Accounts gibt es nur im Bitwarden Secrets Manager, den Vaultwarden nicht
  implementiert (vor dem Bauen in der eigenen Version verifizieren).
  Für Dispatcher-eigene Secrets (Plane-API-Key, Webhook-Secret): `sops` + `age`,
  Key auf dem Server, Datei verschlüsselt im Repo.

## Secrets-Konzept

Claude braucht die **Werte** fast nie — er braucht „welches Secret, wo benutzt, wie
geholt". `SECRETS.md` pro Projekt liefert genau das:

| Secret | Woher | Verwendet in | Zweck |
|---|---|---|---|
| `STRIPE_SK` | `bw get password shop-stripe-live` | `.env` → `config/payment.php` | Zahlungen, LIVE |

Werte erst beim echten Deploy, als bewusster Schritt mit Mensch davor. Vaultwarden
bleibt für die interaktiven Logins zuständig.
