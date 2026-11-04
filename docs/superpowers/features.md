# Funktionsliste

Lebendes Dokument. Stand 2026-11-04.

Jede Funktion hat eine ID (A1, B2, …) — damit lässt sich gezielt zu-/absagen,
ohne die Formulierung zu wiederholen. Neue Ideen hinten im jeweiligen Bereich
anhängen.

**Status-Marker**
- `[da]` — implementiert, läuft
- `[teil]` — Teile im Code, nicht fertig
- `[umbau]` — existiert, muss durch die Client-Server-Architektur geändert werden
- `[plan]` — beschlossen, noch nicht gebaut
- `[vorschlag]` — von mir, nicht von dir; Zu-/Absage offen
- `[nein]` — bewusst verworfen, Begründung dabei

Architektur und Scope-Sortierung: siehe [`ideas.md`](ideas.md).

---

## A — Verbindung & Authentifizierung

- **A1** `[da]` SSH-Verbindung zum Server, mehrere Sessions über eine gemultiplexte
  Verbindung (`ssh/connection.rs`)
- **A2** `[da]` Host-Key-Verifikation mit Fingerprint-Dialog bei unbekanntem Host
  (`ssh/hostkey.rs`) — bleibt lokal gespeichert, bewusst
- **A3** `[da]` Mehrere Verbindungsprofile / Arbeitsbereiche (`NamedProfile`)
- **A4** `[umbau]` Passwort im Windows-Credential-Store merken — widerspricht
  „Client speichert nichts", wird zurückgebaut (`keyring` raus)
- **A5** `[teil]` → `[plan]` **Key-Auth**. `AuthMethod` und `key_path` existieren,
  `connection.rs:98` liest sie. Durch die Architektur jetzt Kernfeature: der Key
  *ist* der Schlüssel, auf Stick/YubiKey haltbar.
- **A6** `[da]` Auto-Reconnect nach Verbindungsverlust, Sessions bleiben erhalten
  (`reconnect_supervisor.rs`, `termPool.ts`)
- **A7** `[vorschlag]` **2FA für den Login** via SSH-Key + TOTP (`pam_oath`)
  serverseitig. Relevant, sobald der Server von außen erreichbar ist.
- **A8** `[vorschlag]` **Verbindung über Tailscale/WireGuard** statt Port 22 öffentlich.
  Kein offener Port im Internet, kein Brute-Force-Vektor, Fail2Ban erübrigt sich
  weitgehend. Empfehlung, falls der Server nicht im LAN steht.

## B — Sessions & Terminal

- **B1** `[da]` Laufende tmux-Sessions auflisten, Status ●/○
- **B2** `[da]` Session als vollwertiges Terminal öffnen, mehrere parallel im
  Hintergrund aktiv
- **B3** `[da]` Session im Projektordner starten („+ startbar")
- **B4** `[da]` Model/Effort pro Session wählen (`claude_invocation`, Befehls-Panel)
- **B5** `[da]` Statusicon je Session: arbeitet / fertig / verloren
- **B6** `[da]` Badge-Zähler + Desktop-Notification bei Output in inaktiver Session
- **B7** `[da]` Scrollback durchsuchen (Strg+F)
- **B8** `[da]` Terminal-Themes und Darstellungsoptionen
- **B9** `[da]` Session beenden (`kill_session`)
- **B10** `[da]` Session läuft weiter, wenn der Client zugeht — tmux-Eigenschaft,
  aber ein Kernargument der Architektur: Rechner zu, Arbeit läuft
- **B11** `[plan]` Aufgaben-Sessions in eigenem `git worktree` je Task, damit
  parallele Sessions sich nicht gegenseitig überschreiben
- **B12** `[vorschlag]` **Hängende Session erkennen.** Claude wartet auf eine Eingabe
  (meist ein Permission-Prompt) → Session steht unbemerkt still. Die Erkennung
  existiert fast schon: `lastOutputAt` ohne neuen Output + Prozess lebt = wartet.
  Nach N Minuten Notification und Kommentar am Plane-Task. Das ist das häufigste
  reale Problem bei unbeaufsichtigten Sessions.
- **B13** `[vorschlag]` **Transkript serverseitig mitschreiben.** tmux-Scrollback ist
  begrenzt, nach Session-Ende ist der Verlauf weg. `claude --output-format
  stream-json` parallel in eine Datei → nachvollziehbar, und die Projekt-Historie
  (C4) kann daraus gespeist werden statt aus Claudes Gedächtnis.
- **B14** `[vorschlag]` **Laufzeit-/Turn-Limit pro Task mit hartem Kill.** Autonome
  Sessions aus Plane verbrauchen Tokens ohne Aufsicht; eine Session in einer Schleife
  läuft sonst bis zum Limit. Pro Projekt konfigurierbar. Das baut man sonst nach der
  ersten Rechnung.
- **B15** `[vorschlag]` **Rate-Limit-Erkennung.** Läuft die Session gegen ein
  Nutzungslimit, soll sie den Plane-Task zurück auf „Todo" legen und das vermerken,
  statt stillzustehen und als „in Arbeit" zu gelten.

## C — Projekte

- **C1** `[da]` Projektverzeichnisse aus konfigurierten Pfaden scannen
  (`cmd_scan_projects`, Marker-basiert)
- **C2** `[plan]` **Projekt anlegen**: Ordner + `git init` + `CLAUDE.md` + `SECRETS.md`
  aus Template + langlebige Projekt-Session. Shell-Script, kein App-Feature.
- **C3** `[plan]` `CLAUDE.md`-Template: „arbeite in diesem Pfad", Frameworks/Tools
  auflisten, Übernahmen aus anderen Projekten hierher kopieren **und committen**
  (im Worktree sonst beim Merge verloren), Historie führen
- **C4** `[plan]` Projekt-Historie als Abschnitt in `CLAUDE.md`, neueste oben, mit Datum
- **C5** `[plan]` Neue Aufgabe → neue Session im Projektpfad (via B11 im Worktree)
- **C6** `[vorschlag]` **Permission-Modus und `allowedTools` pro Projekt**, nicht global.
  Ein Kundenprojekt mit Live-Zugängen darf nicht denselben Bash-Vollzugriff haben wie
  ein Spielprojekt. Liegt in der Server-Config, wird beim Session-Start gesetzt.
  Direkt sicherheitsrelevant.
- **C7** `[vorschlag]` **Review-Gate: kein Push auf den Hauptbranch.** Autonome Session
  arbeitet im Worktree auf `task/<ISSUE>`, Ergebnis ist immer ein PR. Branch-Protection
  serverseitig. Ohne das schreibt Claude unbeobachtet in Kundenprojekte.
- **C8** `[vorschlag]` **Projektübergreifende Suche** über alle Projekte und die
  Wissensbasis. Das ist die praktische Einlösung der „Querverlinkung" — ohne Suche
  bleibt sie eine Sammlung von `@`-Imports, die den Kontext flutet.
- **C9** `[vorschlag]` **Projekt archivieren**: Session beenden, Worktrees aufräumen,
  Ordner markieren, aus „startbar" ausblenden. Sonst wächst die Sidebar unbegrenzt.

## D — Aufgaben & Plane-Anbindung

- **D1** `[plan]` Dispatcher: Plane-Webhook → Session im Projektpfad starten
  (`crates/claudedeck-dispatch`)
- **D2** `[plan]` Webhook-Signaturprüfung (Schema versionsabhängig: `X-Plane-Signature`
  HMAC-SHA256 oder Svix — in der eigenen Instanz verifizieren)
- **D3** `[plan]` Projekt-Whitelist im Dispatcher, keine Pfade aus dem Payload
- **D4** `[plan]` State-UUIDs beim Start auflösen und cachen (Webhooks liefern UUIDs,
  „Todo" ist pro Projekt eine andere)
- **D5** `[plan]` Idempotenz über `tmux new-session -A` + deterministischen Namen
  `task-<ISSUE>` — Plane retryt bei non-200
- **D6** `[plan]` `@plane_issue` als tmux-user-option setzen und in `cmd_list_sessions`
  mitlesen; Issue-ID ist separatorfrei, der **Titel** darf nie ins Format-String
- **D7** `[plan]` Sidebar-Gruppe „Aufgaben" als vierte Gruppe
- **D8** `[plan]` Rückkanal: Session endet → Plane-Task auf „Done", PR-Link als Kommentar
- **D9** `[vorschlag]` **Plan-First-Modus über Plane-Label.** Label `auto` = sofort
  arbeiten; Label `plan-first` = erst Plan als Plane-Kommentar, dann auf Freigabe
  warten. Für teure oder riskante Tasks, ohne zwei Systeme zu brauchen.
- **D10** `[vorschlag]` **Fortschritt als Plane-Kommentar** statt nur im Terminal:
  Start, Plan, Commit, PR, Ende. Damit ist der Task auch ohne ClaudeDeck lesbar —
  vom Handy, unterwegs.
- **D11** `[vorschlag]` **Parallelitäts-Limit**, global und pro Projekt. Fünf
  gleichzeitige Sessions auf einem Homelab-Server heißt fünf Claude-Prozesse plus
  Builds. Eine Warteschlange ist billiger als ein überladener Server.
- **D12** `[nein]` **PM-Funktionen nachbauen** (Terminierung, Kommentare, Zuweisung,
  Team, Notifications, Milestones). Das ist vollständig Plane; Milestones heißen
  dort Cycles/Modules. Nachbauen = Plane in Rust neu schreiben.

## E — Secrets & Tokens

- **E1** `[plan]` `SECRETS.md` pro Projekt: welches Secret, woher, in welcher Datei
  verwendet, wozu — **Referenzen, niemals Werte**
- **E2** `[plan]` Vaultwarden für interaktive Nutzung (`bw get password …`)
- **E3** `[plan]` Dispatcher-eigene Secrets (Plane-API-Key, Webhook-Secret) über
  `sops` + `age`, Key auf dem Server
- **E4** `[nein]` Vaultwarden für den unattended Dispatcher. API-Key-Login lässt das
  Vault gesperrt, es braucht ein `BW_SESSION`, ein zweites `bw unlock` invalidiert die
  erste Session, und ein dauerhaft entsperrtes Vault auf einem Daemon heißt: wer den
  Server hat, hat alles. Machine Accounts gibt es nur im Bitwarden Secrets Manager,
  den Vaultwarden nicht implementiert.
- **E5** `[vorschlag]` **Secret-Leak-Scan vor jedem Commit** (`gitleaks` als
  pre-commit-Hook im Projekt-Template). Eine autonome Session kann einen Wert in eine
  Datei schreiben, ohne es zu melden. Realer Vektor, billige Absicherung.
- **E6** `[vorschlag]` **`SECRETS.md` gegen die Realität prüfen**: Eintrag ohne
  Verwendung im Code und Verwendung ohne Eintrag beide melden. Hält das Manifest
  ehrlich, sonst verrottet es binnen Wochen.
- **E7** `[vorschlag]` **Secrets nie in die Session-Umgebung**, sondern erst im
  Deploy-Schritt mit Mensch davor. Claude braucht die Werte fast nie — er braucht
  „welches Secret, wo benutzt, wie geholt".

## F — Wissen & Dokumentation

- **F1** `[plan]` Zentrale Wissensbasis `/srv/knowledge/` (standards/, clients/,
  decisions/), eigenes Git-Repo
- **F2** `[plan]` Global geltende Standards per `@`-Import in `~/.claude/CLAUDE.md`
- **F3** `[plan]` Projektspezifische Fragmente per Import im Projekt-`CLAUDE.md`
- **F4** `[plan]` Ab ~20 Dateien auf einen MCP-Server über die Wissensbasis umstellen —
  `@`-Imports landen in *jedem* Prompt und fluten den Kontext
- **F5** `[vorschlag]` **Wiederkehrende Abläufe als Skills** (`~/.claude/skills/`)
  statt als MD-Text. Werden nur bei Bedarf geladen, kosten im Normalfall nichts.
- **F6** `[vorschlag]` **ADRs in `decisions/`**: Entscheidungen wie „SSH statt eigener
  API" oder „Worktree pro Task" mit Begründung festhalten. In sechs Monaten ist sonst
  unklar, warum etwas so ist, und es wird versehentlich zurückgedreht.

## G — Dateien

- **G1** `[da]` SFTP-Ablage lesend: Verzeichnis listen, Vorschau, Download
- **G2** `[teil]` SFTP schreibend / Upload — offener M6-Punkt
- **G3** `[da]` Bildvorschau im Ablage-Panel
- **G4** `[vorschlag]` **Diff-Ansicht für den aktuellen Task**: `git diff` des
  Worktrees im Panel, statt im Terminal zu scrollen. Das ist der Blick, den man beim
  Review tatsächlich braucht.

## H — Beobachtung & Benachrichtigung

- **H1** `[da]` Desktop-Notification bei Output in inaktiver Session, pro Session
  abschaltbar
- **H2** `[da]` Notification-Entprellung (ein Hinweis pro Wartezyklus,
  `notifyScheduler`)
- **H3** `[vorschlag]` **Push aufs Handy über ntfy** (self-hosted, EU, kein
  Google/Apple-Push-Dienst nötig). Für „Task hängt" (B12) und „Task fertig" ist der
  Desktop-Hinweis nutzlos, wenn der Rechner zu ist — was bei dieser Architektur der
  Normalfall ist.
- **H4** `[vorschlag]` **Serverseitiges Audit-Log**: welche Session, welcher Task,
  welche Dateien, welche Laufzeit, welches Ergebnis. Für Nachvollziehbarkeit bei
  Kundenprojekten und zum Debuggen. Aufbewahrungsfrist bewusst festlegen, nicht
  unbegrenzt.
- **H5** `[vorschlag]` **Übersicht statt Sessionliste**: was läuft, was hängt, was
  wartet auf Review, was ist heute fertig geworden. Sobald mehr als drei Tasks
  parallel laufen, ist die flache Liste zu wenig.

## I — Backend & Betrieb

- **I1** `[plan]` `crates/claudedeck-backend`: Config, Profile, Projekt-Metadaten,
  Secrets-Referenzen; Ablage unter `/var/lib/claudedeck/`
- **I2** `[plan]` JSON-Kommandos über den SSH-Kanal statt `tmux list-sessions`-Parsing
  im Client
- **I3** `[plan]` systemd-Units für Backend und Dispatcher
- **I4** `[umbau]` `config.rs`: `config_path`/`load_from`/`save_to` wandern
  serverseitig. Das `Config`-Struct bleibt unverändert — 5 Call-Sites in
  `src-tauri/src/commands/`.
- **I5** `[vorschlag]` **Backup der Serverdaten** (Config, Projekt-Metadaten, Historie,
  Transkripte). Liegt alles nur auf einem Homelab-Server, und genau das war der Punkt
  der Architektur. Ohne Backup ist ein Plattenausfall der Totalverlust.
- **I6** `[vorschlag]` **Health-Endpoint / Status im Client**: läuft das Backend, läuft
  der Dispatcher, wann kam der letzte Webhook an. Ein stillgelegter Dispatcher fällt
  sonst erst auf, wenn ein Task stundenlang nicht anläuft.
- **I7** `[vorschlag]` **Dispatcher unter eigenem Benutzer** mit Zugriff nur auf
  `/srv/projects/`, nicht als root. Er nimmt Daten aus dem Internet an — das ist die
  exponierteste Komponente.

---

## Meine fünf wichtigsten Vorschläge

Nach Verhältnis von Aufwand zu verhindertem Schmerz:

1. **B12** Hängende Session erkennen — häufigstes reales Problem, Erkennung fast schon da
2. **C6** Permission-Modus pro Projekt — Sicherheit, und später schwer nachzurüsten
3. **B14** Laufzeit-Limit mit Kill — verhindert die unangenehme Rechnung
4. **E5** Secret-Leak-Scan — ein Hook, ein realer Vektor geschlossen
5. **C7** Review-Gate — kein autonomer Push in Kundenprojekte

## Offene Fragen

- **Zwei Modi?** „Client speichert nichts, sofern keine Arbeitsumgebung hinterlegt
  ist" — heißt das Remote-Modus (nichts lokal) **und** Lokal-Modus (Projekt auf dem
  Gerät, dortige Ablage erlaubt)? Entscheidet, ob `config.rs` zwei Backends braucht.
- **Plane-Version** der Instanz — bestimmt das Webhook-Signatur-Schema (D2).
- **Projektpfad** auf dem Server — `/srv/projects/`?
- **Mehrbenutzer** oder nur du? Zuweisung und Team (D12) sind in Plane, aber wenn
  mehrere Leute Sessions starten, braucht das Backend eine Rechteebene.
