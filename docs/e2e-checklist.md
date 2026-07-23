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
