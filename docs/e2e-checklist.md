# ClaudeDeck — E2E-Abnahme-Checkliste (Windows)

Manuelle Abnahme für M4/M5 (`docs/superpowers/plans/2026-07-23-claudedeck-m4-m5-ui.md`).
Diese Punkte lassen sich nicht sinnvoll automatisieren (echtes WebView2-Rendering, echte
Windows-Notifications, echter Netzabriss) und wurden bisher nur per `tsc`/`vitest`/
`cargo clippy`/Code-Lektüre verifiziert — siehe „Bekannte Einschränkungen" am Ende sowie die
„Windows-Abnahme-Punkte" in `.superpowers/sdd/task-5-report.md` und `task-6-report.md`.

Ablauf: jede Gruppe der Reihe nach durchgehen, Häkchen setzen. Bei Abweichung: Symptom
notieren (Screenshot falls möglich) statt nur „geht nicht".

## Installation

- [ ] `.msi`-Installer aus dem GitHub-Actions-Artifact (`build.yml`, `claudedeck-windows`)
      lässt sich ausführen und installiert ohne Fehlermeldung
- [ ] Portable `claudedeck.exe` aus demselben Artifact startet direkt, ohne Installation
- [ ] Auf einem Windows 10/11 ohne separat nachinstalliertes WebView2 startet die App trotzdem
      (WebView2 Runtime ist auf Windows 11 vorinstalliert; auf Windows 10 ggf. Bootstrapper
      prüfen) — kein leeres/weißes Fenster, keine „WebView2 nicht gefunden"-Meldung

**Erwartung:** App startet in beiden Varianten sichtbar mit dem ConnectGate-Formular, kein
Absturz, kein leeres Fenster.

## Verbindung

- [ ] Erstverbindung ohne gespeichertes Secret: Passwort-Formular erscheint, Verbinden mit
      korrektem Passwort führt zum Hauptlayout (Sidebar + StatusBar „verbunden")
- [ ] Checkbox „im Windows-Anmeldedaten-Speicher merken" angehakt beim Verbinden benutzt
- [ ] App neu starten: kein Passwort-Formular mehr, Auto-Connect aus dem Keyring (Windows
      Credential Manager) läuft ohne Eingabe durch
- [ ] Falsches Passwort: sichtbare Fehlermeldung, Formular bleibt bedienbar (kein Hänger im
      „busy"-Zustand), **kein** automatischer Retry-Loop (fail2ban-Risiko)

**Erwartung:** Passwort-Flow und Keyring-Auto-Connect funktionieren beide; AuthFailed ist ein
klarer Endzustand ohne Loop.

## Hostkey-Erstkontakt

- [ ] Beim allerersten Verbindungsaufbau zu einem noch unbekannten Host erscheint ein Dialog
      mit dem SHA256-Fingerprint sichtbar als Text
- [ ] Enter im Dialog löst **Abbrechen** aus, nicht Bestätigen (Abbrechen ist Default-Button)
- [ ] „Vertrauen & verbinden" klicken → Verbindung kommt tatsächlich zustande, Fingerprint
      wird in `known_hosts` der App gespeichert
- [ ] Geänderter Host-Key (nur falls gefahrlos auf einem Testhost provozierbar, sonst
      Code-Review-Vertrauen wie in Task-5-Report vermerkt): harte Fehlbox ohne
      Bestätigungsoption, keine Möglichkeit, sich trotzdem zu verbinden

**Erwartung:** Unbekannter Key verlangt aktive Bestätigung mit sicherem Default; geänderter
Key lässt sich in der App nicht wegklicken.

## Claude-TUI-Rendering

- [ ] Farben (ANSI/256/Truecolor) im laufenden `claude`-Prozess werden korrekt dargestellt
- [ ] Box-Zeichnungszeichen (Rahmen/Trennlinien im Claude-TUI) erscheinen sauber, keine
      Ersatzzeichen/Kästchen
- [ ] Maus-Scroll im Terminal scrollt den Claude-TUI-Inhalt bzw. das Scrollback wie erwartet
- [ ] Eingabe von deutschen Sonderzeichen äöü€ im Terminal kommt unverändert im
      Claude-Prozess an
- [ ] AltGr-Zeichen (z. B. @, {, }, [, ], \, |) lassen sich eintippen
- [ ] Emoji-Eingabe/-Anzeige funktioniert ohne Darstellungsfehler

**Erwartung:** Das Terminal verhält sich wie ein vollwertiges TUI-Terminal, keine
Zeichensatz- oder Encoding-Verluste bei deutscher Tastatur.

## Mehrere Sessions parallel

- [ ] 3 oder mehr Sessions gleichzeitig öffnen (angehängt in der Sidebar)
- [ ] Umschalten zwischen Sessions fühlt sich subjektiv sofort an (kein spürbarer
      Aufbau-/Ladezustand, kein Flackern, kein Doppel-Rendering)
- [ ] Nach dem Zurückwechseln zu einer vorher geöffneten Session ist das Scrollback
      unverändert vorhanden (Terminal wurde nicht neu aufgebaut/disposed)
- [ ] Eine Session im Hintergrund erzeugt Ausgabe (z. B. Claude antwortet) → Badge-Zahl an
      der Sidebar-Zeile dieser Session erhöht sich, ohne dass man hinschaut

**Erwartung:** TermPool hält alle Instanzen wirklich am Leben; Umschalten ist reines
Ein-/Ausblenden, kein Re-Attach.

## Windows-Notification

- [ ] Eine Hintergrund-Session produziert Ausgabe und ist danach ca. 2 Sekunden still →
      Windows-Notification erscheint mit Sessionname
- [ ] Aktive (gerade angesehene) Session löst **keine** Notification aus
- [ ] „Benachrichtigungen aus" im Kontextmenü einer Session gesetzt → für diese Session
      kommt danach keine Notification mehr, andere Sessions weiterhin

**Erwartung:** 2-Sekunden-Stille-Heuristik trifft im echten Timing, Opt-out ist pro Session
wirksam.

## Scrollback-Suche (Strg+F)

- [ ] Strg+F über einem aktiven Terminal öffnet die Suchleiste
- [ ] Eingabe eines im Scrollback vorhandenen Begriffs findet und markiert Treffer
- [ ] „Weiter"/Enter (findNext) und Shift+Enter (findPrevious) springen zwischen Treffern
- [ ] Esc schließt die Suchleiste wieder
- [ ] Suchleiste schließt automatisch beim Wechsel auf eine andere Session

**Erwartung:** Suche funktioniert im 10 000-Zeilen-Scrollback-Puffer zuverlässig in beide
Richtungen.

## Fenster-Resize

- [ ] Fenster in der Größe ändern (ziehen) → aktives Terminal passt sich sichtbar an
      (cols/rows), Textumbruch im TUI korrekt an der neuen Breite
- [ ] Während des Ziehens kein spürbares Stottern/Einfrieren durch zu viele IPC-Aufrufe
      (Resize-Events sind gedrosselt, siehe Task-5-Report Fix 3 — kein IPC-Sturm)

**Erwartung:** tmux/Terminal folgen der neuen Fenstergröße flüssig, ohne die App oder die
Verbindung sichtbar zu belasten.

## Projekt starten

- [ ] Ein Eintrag unter „+ startbar" anklicken → Session erscheint umgehend unter
      „angehängt", verschwindet aus „startbar"
- [ ] Neue Session zeigt tatsächlich den gestarteten `claude`-Prozess im richtigen
      Arbeitsverzeichnis

**Erwartung:** `start_project` ist idempotent nutzbar und die Sidebar aktualisiert sich ohne
manuellen Refresh.

## Kontextmenü Detach/Kill

- [ ] Kontextmenü („⋮") an einer angehängten Session öffnen
- [ ] „Detach" trennt die App-Anzeige, die tmux-Session läuft serverseitig weiter (mit
      `tmux ls` auf dem Server oder erneutem Anklicken in der Sidebar nachprüfbar)
- [ ] „Kill" (nach Bestätigungsdialog) beendet die tmux-Session tatsächlich
      (`kill_session`), Sidebar-Eintrag verschwindet

**Erwartung:** Detach ≠ Kill; beide Aktionen wirken sich serverseitig korrekt aus.

## WLAN/VPN-Trennung → Reconnect

- [ ] WLAN oder VPN während offener Sessions trennen → `ReconnectOverlay` mit Countdown
      erscheint, Sidebar wird ausgegraut
- [ ] Aktive Session zeigt währenddessen das „Verbindung verloren"-Banner, Terminal/Scrollback
      bleiben sichtbar erhalten (nicht disposed)
- [ ] Verbindung wiederherstellen (WLAN/VPN wieder an) → automatischer Re-Attach ohne
      manuellen Eingriff, betroffene Sessions wechseln von ⚠ zu ● in der Sidebar
- [ ] Terminal-Inhalt/Scrollback der re-attachten Session ist nach dem Reconnect weiterhin
      vorhanden (kein Neuaufbau, keine Lücke außer der tatsächlichen Ausfallzeit)
- [ ] Manueller „Jetzt neu verbinden"-Button im Overlay funktioniert (löst sofortigen
      Retry-Versuch statt Warten auf den nächsten Backoff-Schritt aus)
- [ ] AuthFailed während eines Reconnect-Versuchs (z. B. Passwort inzwischen geändert)
      beendet die Backoff-Schleife endgültig, **kein** Retry-Loop

**Erwartung:** Reconnect ist die aufwendigste und am wenigsten automatisiert getestete
Baustelle des Plans (Task-6-Report: „Kein echter Reconnect je beobachtet") — hier ist die
sorgfältigste manuelle Prüfung nötig.

## M7 — UTF-8/DE-Layout, Sortierung & Suche, Befehls-Panel, Model/Effort

Spec: `docs/superpowers/specs/2026-07-25-claudedeck-m7-design.md`. Die reine Entscheidungslogik
(`keyboard.ts`, `sessionFilter.ts`, `catalogFilter.ts`, `effort.ts`, `catalog/parser.rs`,
`tmux/commands.rs`) ist unit-getestet; die folgenden Punkte lassen sich nur im echten
Windows-Build mit deutschem Tastaturlayout prüfen.

### Zeichen und Tastatur

- [ ] **Ausgabe:** Umlaute in tmux-Session-Namen und Claude-Code-Ausgaben werden korrekt
      dargestellt (nicht `Ã¤`).
- [ ] **Ausgabe:** Claude Codes Rahmenzeichen erscheinen als durchgehende Linien (nicht `???`
      oder Kraut).
- [ ] **Eingabe:** `ä ö ü ß` direkt getippt landen korrekt im Terminal.
- [ ] **Eingabe AltGr:** `AltGr+Q/E/7/8/9/0/ß/<` ergeben `@ € { [ ] } \ |`.
- [ ] **Eingabe AltGr:** Die Zeichen kommen **einfach** an, nicht doppelt (Guard gegen den
      `keyup`-Durchlauf und die versteckte Textarea in `termPool.ts`).
- [ ] **Tote Tasten:** `` ` ``+Leertaste ergibt `` ` ``; `^`+`a` ergibt `^a`; `´`+`e` ergibt `é`.
- [ ] **Strg bleibt Strg:** `Strg+C` bricht weiterhin ab (erscheint nicht als Buchstabe `c`).
- [ ] **AltGr+F** öffnet **nicht** die Scrollback-Suche; `Strg+F` öffnet sie weiterhin.

### Session-Sortierung und -Suche

- [ ] Suchfeld filtert alle drei Gruppen (Angehängt/Läuft/Startbar) gleichzeitig.
- [ ] Leeres Suchfeld zeigt wieder alles; Gruppen ohne Treffer zeigen `–`.
- [ ] Umlaut-Suche funktioniert (`löffel` findet `cc-loeffelholz` **nicht**, `cc-löffelholz` schon).
- [ ] Sortierung „Name", „Zuletzt aktiv" und „Startzeit" ändern die Reihenfolge sichtbar.
- [ ] Startbare Projekte stehen bei den Zeitsortierungen hinten (sie haben keinen Zeitstempel).

### Befehls-Panel

- [ ] `Strg+B` und die schmale Leiste rechts klappen das Panel auf und zu.
- [ ] Beim Aus-/Einklappen passt sich die Terminalbreite an (kein abgeschnittener Text; der
      `ResizeObserver` in `TerminalHost.tsx` muss die Breitenänderung sehen).
- [ ] Skills, Agents, Befehle und Connectors erscheinen in je einem aufklappbaren Akkordeon.
- [ ] Connectors zeigen den Status aus `claude mcp list` (verbunden `●` / nicht verbunden `○`).
- [ ] Klick auf einen Eintrag fügt `/name ` ins Terminal ein und sendet **kein** Enter.
- [ ] Klick auf einen Agent fügt den nackten Namen ohne `/` ein.
- [ ] Ohne aktive Session sind die Einträge deaktiviert (kein stiller Klick ins Leere).
- [ ] Projektlokale Einträge sind mit `●` markiert und stehen in ihrer Gruppe oben.
- [ ] Sessionwechsel in ein anderes Projekt lädt die projektlokalen Einträge neu.
- [ ] Die Panel-Suche findet Einträge auch über ihre **Beschreibung**, nicht nur den Namen.

### Model und Arbeitsstärke

- [ ] **Zuerst prüfen:** Nimmt `/effort high` das Argument inline entgegen, oder öffnet `/effort`
      einen interaktiven Picker? Bei Picker greift der Fallback aus der Spec (nur das nackte
      Kommando einfügen) — dann sind die beiden folgenden Punkte entsprechend anzupassen.
- [ ] Model-Auswahl fügt `/model <name> ` in die aktive Session ein.
- [ ] Der Stärke-Regler fügt `/effort <stufe> ` ein und zeigt die Stufe im Label an.
- [ ] Nach App-Neustart steht die zuletzt gewählte Vorgabe wieder im Regler (aus `config.json`).
- [ ] Eine neu über „+ startbar" gestartete Session läuft mit den Flags — auf dem Server prüfbar
      mit `ps -ef | grep claude` (erwartet: `claude --model … --effort …`).
- [ ] Eine `config.json` **ohne** die neuen Felder lädt weiterhin ohne Fehler.

## M8 — Layout, Themes, Profile, Einstellungen

Spec: `docs/superpowers/specs/2026-07-25-claudedeck-m8-design.md`. Layout, Kontraste und die
Farbschemata wurden vorab per Headless-Rendering geprüft (Screenshots im Entwicklungsverlauf);
die folgenden Punkte brauchen den echten Windows-Build.

### Fenster und Layout

- [ ] **Kein Fenster-Scrollbalken**, bei keiner Fenstergröße — weder horizontal noch vertikal.
- [ ] Nur Sidebar, Befehls-Panel und der Terminalinhalt scrollen; das Fenster bleibt stehen.
- [ ] Sehr lange Session-Namen werden mit „…" gekürzt, nicht abgeschnitten, und erzeugen keine
      horizontale Scrollbar in der Sidebar.
- [ ] Fenster sehr schmal und sehr breit ziehen — nichts überlappt.

### Sidebar

- [ ] Leere Gruppen sind **unsichtbar**; sind alle leer, erscheint ein erklärender Hinweis.
- [ ] Das ⋮ erscheint bei Hover **und** wenn man mit Tab in die Zeile springt.
- [ ] Sekundärtext (Gruppentitel, Beschreibungen) ist gut lesbar — die Farbe wurde von 4,26:1
      auf 5,20:1 angehoben.
- [ ] Jedes bedienbare Element zeigt beim Tabben einen sichtbaren Fokusring.

### Session-Status

- [ ] Eine arbeitende Session zeigt einen **pulsierenden** Punkt.
- [ ] Nach ~2 s Ruhe wechselt sie auf einen **grünen Haken** („fertig, wartet auf Eingabe").
- [ ] Eine Session mit verlorener Verbindung zeigt ⚠ und **keinen** Haken.
- [ ] Bei aktivierter Systemeinstellung „Animationen reduzieren" pulsiert nichts, der Zustand
      bleibt an der Farbe erkennbar.

### Terminal-Themes

- [ ] Jedes der sechs Schemata ändert die Terminalfarben **und** den App-Akzent (Sidebar-Auswahl,
      Badges, Fokusringe).
- [ ] Schriftart JetBrains Mono wird angezeigt, **ohne** dass die App ins Netz geht.
- [ ] Schriftgröße ändern → tmux-Geometrie stimmt weiter: kein Umbruch an falscher Stelle, kein
      abgeschnittener Text. Auch nach dem Wechsel zu einer **anderen, gerade unsichtbaren**
      Session muss deren Darstellung stimmen.
- [ ] `Strg +`, `Strg −`, `Strg 0` wirken sofort; die Größe überlebt einen Neustart.

### Profile

- [ ] Eine `config.json` aus M7 (ohne `profiles`) startet ohne Fehler; das alte Ziel erscheint als
      Profil und das **gespeicherte Passwort funktioniert weiterhin** (die Migration behält die
      ID `default`, unter der es im Anmeldedaten-Speicher liegt).
- [ ] Zweites Profil anlegen, Host/Benutzer/Port setzen, verbinden.
- [ ] Zwischen zwei Profilen wechseln — jedes merkt sich **sein eigenes** Passwort.
- [ ] „Passwort vergessen" entfernt nur das des gewählten Profils.
- [ ] Profil löschen entfernt auch dessen Passwort; das letzte Profil lässt sich nicht löschen.
- [ ] Auto-Connect aus → der Startdialog erscheint, obwohl ein Passwort gespeichert ist.
- [ ] Unbekannter Host-Key → weiterhin Fingerprint-Dialog; geänderter Host-Key → weiterhin nur
      Warnung ohne Bestätigungsmöglichkeit.

### Einstellungen

- [ ] Zahnrad in der Statusleiste und `Strg+,` öffnen den Dialog, Esc schließt ihn.
- [ ] Jede Änderung wirkt sofort und überlebt einen Neustart (kein „Übernehmen"-Knopf).
- [ ] Projektordner bearbeiten ändert die Liste unter „Startbar".

## M9 — Projektfilter, Sortierung, Ablage

Spec: `docs/superpowers/specs/2026-07-25-claudedeck-m9-design.md`. Der Projektfilter und das
Locale-Setup wurden gegen den echten Server ausgeführt; der Rest braucht den Windows-Build.

### Startbar

- [ ] Es erscheinen **nur echte Projekte**, keine Docker-Datenverzeichnisse (auf Isekai: 9 statt
      88 Einträge).
- [ ] Ein neu angelegter Ordner mit `.git` taucht nach dem nächsten Laden auf.
- [ ] Merkmalsliste in den Einstellungen leeren → alle Ordner erscheinen wieder (Filter aus).
- [ ] Sortierung „Zuletzt aktiv" ordnet die Projekte **sichtbar** um; das zuletzt bearbeitete
      steht oben (vorher wirkungslos, weil Projekte keinen Zeitstempel hatten).

### Umlaute — die M7-Zusage, diesmal wirklich

Der bisherige Fix setzte `LC_ALL=C.UTF-8`; diese Locale existiert auf dem Server **nicht**, womit
glibc still auf ASCII zurückfiel. Jetzt wird zur Laufzeit eine vorhandene UTF-8-Locale gewählt.

- [ ] Umlaute in der Ausgabe korrekt, Claude Codes Rahmenzeichen als durchgehende Linien.
- [ ] `ä ö ü ß` getippt kommen im Terminal an.
- [ ] Keine `setlocale`-Warnungen in der Ausgabe.

### Ablage

- [ ] Reiter „Ablage" im rechten Panel öffnet den Ordner der aktiven Session.
- [ ] Ordner lassen sich betreten, „↑" führt zurück; an der Wurzel ist „↑" deaktiviert.
- [ ] Ein von Claude erzeugtes PNG steht **oben** (neueste zuerst) und zeigt beim Klick eine
      Vorschau.
- [ ] Eine Nicht-Bild-Datei anklicken lädt sie direkt herunter.
- [ ] Download landet im Downloads-Ordner; der Pfad erscheint als Hinweis.
- [ ] **Zweimal dieselbe Datei laden erzeugt „ (2)"** statt die erste zu überschreiben.
- [ ] Eine Datei über 8 MB zeigt keine Vorschau, sondern eine Meldung — und stürzt nicht ab;
      der Download funktioniert trotzdem.
- [ ] Sessionwechsel in ein anderes Projekt wechselt den Ordner der Ablage.
- [ ] Ohne offene Session erklärt die Ablage das, statt leer oder kaputt zu wirken.
- [ ] Ein Ordner mit vielen Dateien scrollt **innerhalb** des Panels; das Fenster bleibt stehen.

### Panel-Reiter

- [ ] „Befehle" und „Ablage" lassen sich wechseln; `Strg+B` klappt weiterhin das ganze Panel zu.
- [ ] Model und Arbeitsstärke stehen **nur noch** im Einstellungen-Dialog, nicht mehr im Panel.

## M9a — xterm-Stylesheet (Auswahl-Versatz)

`@xterm/xterm/css/xterm.css` wurde seit M4 **nie eingebunden**. Im Browser nachgemessen (xterm
6.0): die Zeilen standen dadurch 63px zu tief bei 22px Zellenhöhe — 2,9 Zeilen —, während
`SelectionService` weiter ab Oberkante rechnete.

- [ ] Text mit der Maus markieren: die Markierung liegt **genau unter dem Zeiger**, nicht ein
      paar Zeilen darunter.
- [ ] Auch weit unten im Terminal markieren — der Versatz war überall gleich groß, ein Rest
      würde also unten genauso auffallen.
- [ ] Doppelklick markiert das Wort unter dem Zeiger.
- [ ] Terminal-Scrollbalken erscheint innerhalb des Terminals; mit dem Rad scrollt der
      Scrollback, nicht das Fenster.
- [ ] Auswahl kopieren und einfügen liefert den markierten Text.

## Bekannte Einschränkungen / noch nicht abgenommen

- **Auth::Key-Pfad ungetestet.** `ConnectGate` unterscheidet aktuell nicht zwischen
  Passwort- und Key-Auth im Formular (Task-5-Report: „falls ein Profil auf Key-Auth steht,
  zeigt das Formular trotzdem ein Passwort-Feld, das dann ungenutzt bleibt"). Ein Profil mit
  `auth: "Key"` (SSH-Key + Passphrase aus dem Keyring) ist auf keiner Plattform bisher
  durchgespielt worden — weder die Passphrase-Eingabe noch der eigentliche Key-basierte
  Verbindungsaufbau.
- **Echter Netzabriss nur hier verifizierbar.** Der komplette Reconnect-Supervisor
  (Keepalive-Trigger, Backoff-Timing 3/6/12/30s, `PtyEvent::Exit(None)`-Heuristik bei totem
  Transport, server-seitiges Re-Attach über eine neue SSH-Verbindung) wurde ausschließlich
  gegen `cargo check`/`clippy`/`fmt` und isolierte reine Zeitlogik (`attempt_delay`)
  verifiziert, nie gegen einen echten Verbindungsabbruch. Insbesondere unklar: ob ein
  Netzausfall zuverlässig `read_half.wait()` ohne `ExitStatus` enden lässt (statt beliebig
  lange zu hängen) — hängt von russh/TCP-Timeout-Verhalten ab, das sich nur auf einem echten
  Windows-Client mit echtem Netzabriss beobachten lässt.
- **Hostkey-Changed-Fall** lässt sich in einer Dev-Umgebung schwer sicher provozieren
  (erfordert einen echten Host-Key-Wechsel auf `isekai.local`) — nur wenn gefahrlos
  reproduzierbar testen, sonst bleibt es bei Code-Review-Vertrauen.
- **SFTP-Panel existiert noch nicht** (kommt erst mit M6) — kein Abnahmepunkt in dieser
  Checkliste.
- **Kein Windows-Build/Display während der Implementierung verfügbar** (Entwicklung läuft
  auf einem Linux-Host ohne Rust-Toolchain, alle Rust-Kommandos über `./dev.sh cargo …`) —
  jeder Punkt oben ist daher der erste tatsächliche Interaktionstest der jeweiligen Funktion.
