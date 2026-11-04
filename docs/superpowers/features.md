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

- **A9** `[plan]` **Credentials-Bereich** in der Oberfläche: SSH-Zugänge und Tokens
  zentral verwalten. Ablage serverseitig (I1), Werte nie zum Client.
- **A10** `[plan]` **Rechte/Scopes je Zugang anzeigen — immer manuell eintragbar**,
  automatische Ermittlung wo möglich als Komfort obendrauf. Jeder Eintrag trägt einen
  Herkunftsmarker (manuell / abgefragt) und ein Prüfdatum, sonst zeigt das Feld
  irgendwann Fiktion. Freitext zusätzlich, für Dinge wie „nur Repo X" oder „darf nicht
  löschen", die kein Scope-Modell abbildet:
  - GitHub **classic** PAT: Scopes stehen im `X-OAuth-Scopes`-Response-Header → automatisch
  - GitHub **fine-grained** PAT: liefert diesen Header nicht. `X-Accepted-GitHub-Permissions`
    sagt nur, was der *Endpoint verlangt*, nicht was der Token *hat*. Keine API für die
    eigenen Rechte → manuelles Feld oder Endpoint-Probing (hässlich)
  - Plane-API-Key: kein Scope-Konzept, immer workspace-weit → festes Feld
  - SSH-Key: hat selbst keine Rechte. Relevant sind die `authorized_keys`-Optionen und
    die Unix-Rechte des Users → `sudo -l` und Gruppenzugehörigkeit abfragen
- **A11** `[vorschlag]` **Ablaufdatum und Rotationshinweis je Zugang.** GitHub-PATs
  laufen ab, iCloud-Trust-Token nach 30 Tagen. Ein abgelaufener Token in einer
  unbeaufsichtigten Session äußert sich als rätselhafter Fehler mitten im Task.
- **A12** `[vorschlag]` **„Letzter erfolgreicher Einsatz" je Zugang.** Findet toten
  Ballast — Tokens, die noch Rechte haben, aber niemand mehr braucht.

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

- **B16** `[plan]` **Console**: freies Terminal ohne Task-Bindung, für Handgriffe auf
  dem Server. Wichtig: als normale tmux-Session (`console`) anlegen, nicht als
  Sonderweg — sonst greifen Auto-Reconnect (A6) und „läuft weiter, wenn der Client
  zugeht" (B10) nicht.
- **B17** `[vorschlag]` Mehrere benannte Konsolen statt einer. Kostet fast nichts,
  weil es dieselbe Mechanik ist, und eine Konsole ist immer zu wenig.
- **B18** `[plan]` **Session-Übersicht aktiv/inaktiv + Session-Archiv pro Projekt.**
  Das Archiv sind die mitgeschriebenen Transkripte — setzt B13 voraus, sonst ist nach
  Session-Ende nichts mehr da, was man archivieren könnte.

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

- **C10** `[vorschlag]` **Herkunfts-Index über alle Projekte.** Die `CLAUDE.md`-Regel
  „kopiere es hierher und führe eine Historie" (C3) erzeugt Kopien und damit Drift:
  Ein Bugfix in Projekt A erreicht B und C nicht, weil niemand weiß, dass sie dieselbe
  Kopie tragen. Ein Index aus den Historie-Einträgen beantwortet beide Richtungen —
  „woher kam das" und „wer benutzt es noch". Das ist die tragfähige Einlösung der
  Querverlinkung; `@`-Imports allein leisten sie nicht.
- **C11** `[vorschlag]` **Projekt-Vorlagen** (WordPress, PHP-Tool, statische Seite):
  vorgefertigte `CLAUDE.md`, `SECRETS.md`, `.gitignore`, pre-commit-Hooks,
  `allowedTools`-Profil. C2 legt *ein* Projekt an, das legt den *richtigen* an.
- **C12** `[vorschlag]` **Read-only-Projekte.** Session darf lesen und analysieren,
  aber nichts schreiben — für Audits und Recherche in fremdem Kundencode. Ergänzt C6.

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

- **B19** `[vorschlag]` **Kill-Switch**: alle Sessions auf einmal stoppen. Bei einer
  entgleisten Session will man einen Knopf, nicht acht Terminals durchklicken.
- **B20** `[vorschlag]` Session umbenennen aus der Oberfläche, ohne tmux-Kommando
- **B21** `[vorschlag]` Terminal-Ausgabe als Datei exportieren — zum Weitergeben eines
  Fehlers, ohne aus dem Scrollback zu kopieren

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

- **G5** `[vorschlag]` Upload per Drag & Drop, sobald G2 steht
- **G6** `[vorschlag]` Pfad-Lesezeichen je Projekt (`uploads/`, `logs/`) — erspart
  das Durchklicken bei jedem Öffnen

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

- **H6** `[plan]` **Auslastungsanzeige** des Servers: CPU, RAM, Disk, Load, und wie
  viele Sessions gerade laufen
- **H7** `[vorschlag]` **Auslastung als Bremse, nicht nur als Anzeige.** Über einer
  Schwelle startet kein neuer Task, sondern wandert in die Warteschlange (D11). Eine
  Anzeige, die man ignoriert, verhindert keinen überladenen Server.

- **H8** `[vorschlag]` **Watchdog für sensible Pfade.** Ändert eine Session
  `wp-config.php`, `.env`, `.htaccess`, `composer.json`, `package.json` oder etwas
  unter `.github/workflows/`, kommt sofort ein Hinweis — unabhängig vom Task-Ende.
  Kurze Liste, und genau die Dateien, bei denen ein stiller Fehler teuer wird.
- **H9** `[vorschlag]` **Laufzeit je Task** erfassen. Für die Kundenabrechnung: die
  echte Session-Zeit kennt nur ClaudeDeck, Plane sieht nur Status-Wechsel.
- **H10** `[vorschlag]` **Token-Kosten je Task, Projekt und Monat.** Fällt mit dem
  Limit aus B14 zusammen — wer kappen will, muss ohnehin zählen.

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

- **I8** `[plan]` **Zwei Modi: Remote und Lokal.** Remote = nichts auf dem Gerät außer
  Host/Port, SSH-Key und Host-Key-Fingerprint. Lokal = Arbeitsumgebung liegt auf dem
  Gerät, dortige Ablage erlaubt. `config.rs` bekommt dafür ein Trait `ConfigStore` mit
  `LocalStore` und `RemoteStore`; das `Config`-Struct bleibt für beide dasselbe.

## J — Archiv & Archivierung

- **J1** `[plan]` Projekt archivieren mit Abfrage: **Files ins GitHub-Repo oder lokal**
- **J2** `[plan]` Packen + Doku + definierte Benennung
  (Vorschlag: `<projekt>_<YYYY-MM-DD>_<kurzbeschreibung>.tar.zst`)
- **J3** `[plan]` **Archivpfad separat und frei wählbar** — über `rclone`. Ein Tool
  deckt Drive, OneDrive, SharePoint, Nextcloud, S3, SFTP und externe Datenträger ab;
  jeweils eigene APIs und OAuth-Flows zu bauen wäre ein Projekt für sich. Die
  rclone-Config enthält Tokens → gehört serverseitig und verschlüsselt.
- **J4** `[nein]` **iCloud als Archivziel** — abgesagt 2026-11-04. rclone hat das Backend seit v1.69, aber
  der Trust-Token läuft nach 30 Tagen ab und braucht dann erneut 2FA von Hand,
  App-spezifische Passwörter werden abgelehnt, und Advanced Data Protection muss
  ausgeschaltet sein — ein Sicherheits-Downgrade des gesamten Apple-Kontos für ein
  Archiv. Apple hat den Zugang mehrfach gebrochen. Für Automatisiertes unbrauchbar.
- **J5** `[vorschlag]` **Verschlüsselung vor dem Upload** (`rclone crypt` oder `age`).
  Sieht der Anbieter nur Chiffrat, ist der Drittlandtransfer praktisch entschärft und
  OneDrive/SharePoint/Drive datenschutzrechtlich fast gleichgültig. Der Schlüssel
  liegt beim Server und **niemals mit im Archiv**.
- **J6** `[vorschlag]` Default-Ziel Nextcloud self-hosted oder Hetzner Storage Box
  (EU, AVV unkompliziert). US-Dienste nur zusammen mit J5.
- **J7** `[plan]` **Manifest und Doku neben das Archiv, nicht hinein.** Liegt die Doku
  im Tarball, muss das Dashboard jedes Archiv entpacken, nur um Name und Beschreibung
  anzuzeigen. Entscheidet über die Brauchbarkeit von J9.
- **J8** `[plan]` Manifest-Inhalt: Projektname, Datum, Git-Remote und letzter Commit,
  Branch, Session-Konfiguration, Secrets-**Referenzen**, Doku-Text, Checksumme,
  **Format-Version**. Ohne Versionsfeld sind alte Archive später nicht mehr sicher
  reaktivierbar.
- **J9** `[plan]` **Archiv-Dashboard**: alle Archive mit Namen und Doku, ohne Entpacken
- **J10** `[plan]` **1-Click reaktivieren**: herunterladen, entpacken, Pfad
  wiederherstellen, Session starten. Setzt J8 voraus.
- **J11** `[vorschlag]` **Git-Repo und Tarball nicht verwechseln.** Hat das Projekt ein
  Remote, ist „pushen und Ordner löschen" das Archiv — vollständig und durchsuchbar.
  Der Tarball ist für das, was *nicht* in Git liegt: Uploads, Assets, `.env`-Vorlagen,
  lokale Datenbanken. Beides anbieten, aber getrennt benennen, sonst entstehen Archive,
  die die Historie doppelt und die Uploads vergessen.
- **J12** `[vorschlag]` Integritätsprüfung beim Reaktivieren gegen die Checksumme aus J8
- **J13** `[vorschlag]` **Archiv-Test quartalsweise**: ein Archiv probeweise
  reaktivieren. Ein nie getestetes Archiv ist kein Archiv, sondern eine Hoffnung.
- **J14** `[vorschlag]` Aufbewahrungsfrist je Projekt. Kundenprojekte mit
  personenbezogenen Daten gehören nicht unbegrenzt ins Archiv.

## K — Claude-Funktionen in der Oberfläche

- **K1** `[da]` Slash-Commands und MCP-Connectors scannen (`catalog/`)
- **K2** `[plan]` **Agent-Übersicht**: welche Agents konfiguriert sind
  (`.claude/agents/` im Projekt, `~/.claude/agents/` global)
- **K3** `[plan]` **Welche Agents aktiv sind.** Haken: Subagents laufen *innerhalb* der
  Claude-Session und sind von außen nicht sichtbar — über tmux oder die Prozessliste
  nicht ermittelbar. Geht nur aus dem Transkript, **braucht also B13**.
- **K4** `[vorschlag]` Skills-Übersicht, analog K2
- **K5** `[vorschlag]` **MCP-Connector-Status** (verbunden / fehlgeschlagen / Auth
  nötig). Ein nicht verbundener Connector scheitert sonst stumm mitten im Task, und
  die Session wirkt nur unerklärlich unfähig.
- **K6** `[vorschlag]` **Permission-Prompts in der Oberfläche beantworten** statt im
  Terminal zu suchen, welche von acht Sessions gerade fragt. Direkte Ergänzung zu B12.

- **I9** `[vorschlag]` **Read-only-Weboberfläche für unterwegs.** Sobald Push aufs
  Handy läuft (H3), will man darauf reagieren können: Status sehen, Permission-Prompt
  beantworten, Task freigeben. Das ist der einzige Grund, der später doch eine
  HTTPS-API rechtfertigt — dann aber additiv neben SSH, nicht als Ersatz.

## L — Bedienung & Navigation (QoL)

- **L1** `[vorschlag]` **Command Palette** (Strg+K): Projekt wechseln, Session öffnen,
  Task anlegen, Archiv suchen — alles über eine Eingabe. Ab etwa 15 Projekten ist
  Klicken verloren.
- **L2** `[vorschlag]` **Globale Suche** über Transkripte (B13), Projekte,
  Wissensbasis und Archiv-Manifeste. „Wie habe ich das damals gelöst" ist die
  häufigste Frage an so ein System und derzeit unbeantwortbar.
- **L3** `[vorschlag]` Hotkeys für den Sessionwechsel (Strg+1…9)
- **L4** `[vorschlag]` **Mini-Preview in der Sidebar**: die letzten zwei Zeilen je
  Session. Bei acht parallelen Sessions sieht man ohne Klick, welche arbeitet und
  welche wartet. Billig, weil der Output ohnehin durchläuft.
- **L5** `[vorschlag]` **Split-View**: zwei Sessions gleichzeitig sichtbar. Beim
  Vergleichen zweier Tasks oder Terminal neben Diff.
- **L6** `[vorschlag]` **Notizfeld je Projekt und Session**, serverseitig. „Hier war
  ich stehengeblieben" — das, was man sonst in eine Textdatei schreibt und verliert.
- **L7** `[vorschlag]` **Tagesstart-Ansicht**: was nachts gelaufen ist, was auf Review
  wartet, was hängt, was heute fertig wurde. Bei „Rechner zu, Arbeit läuft" ist das
  die erste Ansicht, die man morgens braucht. Konkretisiert H5.

## M — Wartung & Wiederkehrendes

Der Loop reagiert auf Tasks. Dieser Bereich **erzeugt** sie — wiederkehrende Arbeit,
die bei Kundenbetreuung den größeren Teil ausmacht und gerade deshalb vergessen wird.
Der Dispatcher legt die Tasks selbst in Plane an und arbeitet sie ab.

- **M1** `[vorschlag]` **Geplante Tasks pro Projekt**: WordPress-Core- und
  Plugin-Updates prüfen, Backup-Verifikation, Broken Links, Core Web Vitals. Ergebnis
  als Plane-Task mit Befund, nicht als stiller Cron-Lauf.
- **M2** `[vorschlag]` **Ablauf-Überwachung**: SSL-Zertifikate, Domains, API-Tokens
  (A11). Alles mit Datum, das unbemerkt verfällt und dann am Wochenende auffällt.
- **M3** `[vorschlag]` **Dependency- und CVE-Prüfung** je Projekt (`composer audit`,
  `npm audit`, WordPress-Plugin-Schwachstellen). Befund als Task, nicht als Mail,
  die man wegklickt.
- **M4** `[vorschlag]` **PHP-/Runtime-EOL-Warnung**: welches Projekt läuft auf einer
  Version, die aus der Sicherheitswartung fällt. Mit Vorlauf, nicht am Stichtag.
- **M5** `[vorschlag]` **Projekt-Health-Übersicht**: pro Projekt letzter Commit,
  offene Tasks, veraltete Dependencies, Runtime-EOL, SSL-Ablauf, letzte
  Backup-Prüfung. Zeigt, welcher Kunde Aufmerksamkeit braucht — statt es zu merken,
  wenn er anruft.
- **M6** `[vorschlag]` **Wartungsbericht je Kunde**, monatlich, aus M1–M5. Was geprüft,
  was gefunden, was behoben wurde. Belegt Arbeit, die sonst unsichtbar bleibt.

---

## Meine fünf wichtigsten Vorschläge

Nach Verhältnis von Aufwand zu verhindertem Schmerz:

1. **B12** Hängende Session erkennen — häufigstes reales Problem, Erkennung fast schon da
2. **C6** Permission-Modus pro Projekt — Sicherheit, und später schwer nachzurüsten
3. **B14** Laufzeit-Limit mit Kill — verhindert die unangenehme Rechnung
4. **E5** Secret-Leak-Scan — ein Hook, ein realer Vektor geschlossen
5. **C7** Review-Gate — kein autonomer Push in Kundenprojekte

## Offene Fragen

- ~~Zwei Modi?~~ **Geklärt 2026-11-04**: beide. Siehe I8.
- **Archivziel** konkret — Nextcloud, Hetzner, OneDrive, externer Datenträger? Bestimmt,
  welche rclone-Backends eingerichtet werden (J3).
- **GitHub-Tokens** classic oder fine-grained? Entscheidet, ob die Rechte-Anzeige in
  A10 automatisch geht oder ein Pflegefeld wird.
- **Plane-Version** der Instanz — bestimmt das Webhook-Signatur-Schema (D2).
- **Projektpfad** auf dem Server — `/srv/projects/`?
- **Mehrbenutzer** oder nur du? Zuweisung und Team (D12) sind in Plane, aber wenn
  mehrere Leute Sessions starten, braucht das Backend eine Rechteebene.
