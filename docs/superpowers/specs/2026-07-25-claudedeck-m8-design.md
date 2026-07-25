# ClaudeDeck M8 — Layout-Fix, Design-Politur, Terminal-Themes, Profile & Einstellungen

## Context

M7 (UTF-8/AltGr, Sortierung, Befehls-Panel, Model/Effort) ist gebaut und als Windows-Build
abgenommen. Beim ersten echten Benutzen fielen vier Dinge auf:

1. **Die App-Shell scrollt.** Überall Scrollbars, das ganze Fenster verschiebt sich. Ein
   Desktop-Tool darf nie als Seite scrollen — nur die inneren Bereiche.
2. **Die Sidebar ist unübersichtlich**, das Gesamtbild könnte ruhiger sein.
3. **Das Terminal soll besser aussehen**, mit wählbaren Stilen.
4. **Der Start verlangt nur ein Passwort.** Server und Benutzer stehen fest in der
   `config.json`; mehrere Server lassen sich gar nicht hinterlegen.

Ziel: ein Fenster, das sich wie ein natives Werkzeug anfühlt — nichts wackelt, nichts scrollt
ungewollt, die Oberfläche tritt hinter die Arbeit zurück, und Server/Aussehen sind ohne
Texteditor konfigurierbar.

### Zwei Befunde, auf denen der Plan aufbaut

**Scroll-Ursache (gemessen, nicht vermutet).** `src/index.css:45` setzt `#root { height: 100vh }`,
und **nirgends** im Projekt steht ein `overflow: hidden`. Jedes Überlaufen landet damit auf dem
Dokument statt in einem Pane. Verstärkend: `.terminal-area` (`App.css`) fehlt `min-height: 0`,
wodurch xterms Inhalt die Flex-Zeile über die Containerhöhe aufziehen kann. `100vh` ist im
WebView2 zusätzlich unzuverlässig, sobald eine horizontale Scrollbar auftaucht.

**Kontrastfehler (gerechnet).** `--text-dim` (`#7c7f8c`) auf `--bg-panel` (`#1b1c23`) ergibt
**4,26:1** und verfehlt damit die 4,5:1 für normalen Fließtext. Diese Farbe trägt heute
Gruppentitel, Befehlsbeschreibungen, Statuszeile und Platzhalter — also fast allen Sekundärtext.

**Wiederverwendung statt Neubau.** `crates/claudedeck-core/src/secrets.rs` nimmt in `get`/`set`/
`delete` bereits einen `profile: &str` entgegen; nur `PROFILE_ID = "default"`
(`src-tauri/src/commands/connection.rs:49`) ist hartkodiert. Mehrere Profile brauchen deshalb
**keine** neue Secret-Architektur — nur einen echten Schlüssel statt der Konstante.

---

## Abschnitt 1 — Die Shell hört auf zu scrollen

Kleinster Abschnitt, größte Wirkung. Zuerst umsetzen, damit alles Weitere auf einem stabilen
Rahmen aufsetzt.

- `src/index.css`: `html, body { height: 100%; overflow: hidden; }` und `#root { height: 100% }`
  statt `100vh` (dvh/vh weichen im WebView2 von der Client-Höhe ab).
- `src/App.css`: `.app-body { overflow: hidden }` und `.terminal-area { min-height: 0 }`.
  Ohne das zweite kann ein Flex-Item mit `min-height: auto` seine Zeile aufziehen.
- Scroll-Verantwortung explizit festlegen: **genau drei** Elemente scrollen —
  `.sidebar`, `.command-panel`, und xterms eigener Viewport. Alles andere `overflow: hidden`.
  Als Kommentar in `App.css` festhalten, damit es nicht wieder zerfasert.

---

## Abschnitt 2 — Sidebar aufräumen, Oberfläche beruhigen

Struktur bleibt (drei Gruppen), das Rauschen geht.

- **Leere Gruppen verschwinden** statt „–" anzuzeigen. Nur wenn *alle* leer sind, erscheint ein
  einzelner Leerzustand, der erklärt statt zu schweigen („Keine Session — links ein Projekt
  starten").
- **Das ⋮-Menü erscheint nur bei Hover oder Tastaturfokus** (`:focus-visible` innerhalb der
  Zeile), nicht dauerhaft in jeder Zeile. Es bleibt per Tab erreichbar — Sichtbarkeit über
  `opacity`, nicht `display`, damit der Fokus nicht verloren geht.
- **Statuspunkte vereinheitlichen.** Heute mischen sich `●`, `○`, `+`, `⚠` als Textglyphen in
  unterschiedlichen Größen. Stattdessen ein gemeinsames 6px-Punkt-Element mit Farbrolle
  (angehängt / läuft / verloren) und ein separates `+` nur für startbare Projekte.
- **Vertikale Rhythmik**: durchgehende 4px-Skala (4/8/12/16) statt gemischter Werte; Gruppen
  enger, Zeilen etwas luftiger — das ist der eigentliche Grund, warum es „voll" wirkt.
- **Sortierung als kompakter Icon-Button mit Menü** statt eines vollbreiten `<select>` — spart
  die halbe Kopfzeile und macht Platz für die Suche.
- **Kontrast**: `--text-dim` anheben, bis es auf `--bg` **und** `--bg-panel` ≥ 4,5:1 erreicht;
  eine zweite, noch dunklere Rolle (`--text-faint`) nur für großflächig-dekorative Zwecke
  einführen, damit die Aufhellung nicht überall wirkt.
- **Fokusringe**: einheitlicher `:focus-visible`-Stil für alle interaktiven Elemente (heute nur
  auf den neuen Suchfeldern) — im Terminal-Tool wird viel mit der Tastatur navigiert.

Keine neuen Abhängigkeiten, keine Icon-Bibliothek: das Projekt hat als Rahmenbedingung „ein
CSS-File, keine Lib" (`App.css:1`), das bleibt so.

---

## Abschnitt 3 — Terminal-Themes und Darstellung

**Neu `src/lib/terminalTheme.ts`** — Registry aus benannten Themes, jedes mit dem vollständigen
xterm-`ITheme` (16 ANSI-Farben, Vorder-/Hintergrund, Cursor, Selektion) plus einer
`accent`-Farbe für die App:

- ClaudeDeck Dark (aus der bestehenden Palette abgeleitet, Standard)
- Tokyo Night · Nord · Gruvbox Dark · Solarized Dark · Catppuccin Mocha

**Die App nimmt den Akzent des Themes auf.** Beim Wechsel wird `--accent` und `--accent-bg` per
`document.documentElement.style.setProperty` gesetzt; alles Weitere (Auswahl in der Sidebar,
Badges, Fokusringe) hängt bereits an diesen Variablen. Ein Theme-Wechsel färbt die App also
ohne zusätzliche Verdrahtung mit.

**Darstellungsoptionen**, alle live auf alle gepoolten Terminals anwendbar:
Schriftart, Schriftgröße, Zeilenhöhe, Buchstabenabstand, Cursorform (Balken/Block/Unterstrich),
Cursor-Blinken, Scrollback-Größe.

- **`src/lib/termPool.ts`** bekommt `applyDisplayOptions(opts)`, das über alle Pool-Einträge
  läuft, `term.options.*` setzt, danach `fit()` ruft und die geänderten `cols`/`rows` über den
  bestehenden `onResize`-Pfad ans Backend meldet. **Ohne das `fit()` bleibt nach einer
  Schriftgrößenänderung die tmux-Geometrie falsch** — der häufigste Fehler bei so einer Änderung.
- **Schriftart**: JetBrains Mono als `woff2` lokal mitliefern (`src/assets/`, per `@font-face`
  eingebunden) plus die auf Windows vorhandenen Consolas / Cascadia Mono / Lucida Console zur
  Auswahl. Selbst gehostet, weil ein CDN unter Tauris CSP ohnehin blockiert wäre.
- **Zoom per Tastatur**: `Strg +` / `Strg -` / `Strg 0` verstellen die Schriftgröße und schreiben
  sie in die Config — die mit Abstand am häufigsten gebrauchte Einstellung.

**Bewusst nicht**: das WebGL-Addon. `termPool.ts:11` schließt es mit Begründung aus (YAGNI +
Kontextlimit bei vielen parallelen Terminals); ohne gemessenen Bedarf bleibt das so.

---

## Abschnitt 4 — Mehrere Verbindungsprofile

**Config** (`crates/claudedeck-core/src/config.rs`):

```rust
pub struct NamedProfile {           // id ist stabil und dient als Keyring-Schlüssel
    pub id: String,                 // "default" für das migrierte Altprofil
    pub name: String,               // frei wählbar, z.B. "Isekai" / "VPS Hetzner"
    pub host: String, pub port: u16, pub user: String,
    pub auth: AuthMethod, pub key_path: Option<String>,
}
pub struct Config {
    pub profiles: Vec<NamedProfile>,
    pub active_profile: Option<String>,
    pub auto_connect: bool,
    // profile: Profile bleibt als veraltetes Feld erhalten (Migrationsquelle)
    …
}
```

- **Migration als pure Funktion** `migrate_profiles(cfg) -> Config`: ist `profiles` leer, wird
  aus dem alten `profile` eines mit `id = "default"` erzeugt. Damit bleiben **bestehende
  Keyring-Einträge gültig**, denn die liegen bereits unter dem Profilnamen `"default"`.
  Unit-testbar mit Fixture-JSON, wie `config.rs` es schon durchgängig macht.
- **`PROFILE_ID`-Konstante entfällt.** `connect`, `accept_hostkey_and_connect`, `save_secret` und
  `has_secret` bekommen die Profil-ID als Parameter und reichen sie an den vorhandenen
  `SecretStore` durch — die Signatur passt bereits.
- **`known_hosts` bleibt global.** Das entspricht dem SSH-Modell (Host-Key gehört zum Host, nicht
  zum Profil) und braucht keine Änderung.

**ConnectGate** (`src/components/ConnectGate.tsx`) wird vom reinen Passwortfeld zum Startdialog:
Profilauswahl, Passwort, „merken", „Verbinden" — plus ein Weg zur Profilverwaltung. Ist
`auto_connect` gesetzt und für das aktive Profil ein Secret hinterlegt, verbindet die App wie
bisher sofort durch. Der bestehende Hostkey-Flow (unbekannt → Dialog, geändert → nur Warnung,
kein Auto-Retry) bleibt unverändert; das ist eine Sicherheitszusage aus M2.

---

## Abschnitt 5 — Zentraler Einstellungen-Dialog

**Neu `src/components/dialogs/SettingsDialog.tsx`** mit vier Reitern. Kein Modal-Reflex: der
Dialog ist hier richtig, weil es um selten geänderte, formularartige Konfiguration geht.

| Reiter | Inhalt |
|---|---|
| **Profile** | Liste anlegen/bearbeiten/löschen, aktives setzen, Host/Port/Benutzer/Auth, Passwort merken, Auto-Connect |
| **Terminal** | Farbschema, Schriftart, Größe, Zeilenhöhe, Cursorform + Blinken, Scrollback |
| **Sessions** | `scan_paths` bearbeiten (heute nur von Hand in der `config.json`), Model und Effort |
| **Hinweise** | Benachrichtigungen an/aus, Ruhezeit (`silence_ms`) |

- **Model/Effort ziehen aus dem Befehls-Panel hierher um.** Das Panel behält seine Aufgabe
  (Befehle finden und einfügen) und wird dadurch schlanker.
- Erreichbar über die Statusleiste (Zahnrad) und `Strg+,` — das übliche Kürzel.
- **Tastenkürzel-Übersicht** als eigener kleiner Dialog auf `Strg+?`, weil mit M7/M8 inzwischen
  fünf Kürzel existieren (Strg+F, Strg+B, Strg+,, Strg+±, Strg+?).

---

## Reihenfolge

1. **Abschnitt 1** (Scroll-Fix) — eigenständig, sofort spürbar, Grundlage für alles Weitere.
2. **Abschnitt 3** (Themes) — liefert die Farbrollen, auf die Abschnitt 2 aufsetzt.
3. **Abschnitt 2** (Sidebar/Politur) — mit den finalen Farben.
4. **Abschnitt 4** (Profile) — Backend zuerst, dann ConnectGate.
5. **Abschnitt 5** (Einstellungen) — bündelt, was 3 und 4 eingeführt haben.

---

## Verifikation

**Automatisiert — an `ci.yml` ausgerichtet, nicht an dieser Liste.** Das war der Fehler in M7:
`cargo fmt` und `clippy` wurden übersehen, weil ich meiner eigenen Planliste statt der CI folgte.

```bash
./dev.sh cargo fmt --all --check
./dev.sh cargo clippy -p claudedeck-core --all-targets -- -D warnings
./dev.sh cargo test -p claudedeck-core
./dev.sh cargo check -p app
node_modules/.bin/tsc -b tsconfig.json && node_modules/.bin/vitest run && node_modules/.bin/oxlint src
```

Neue Unit-Tests für die pure Logik: `migrate_profiles` (Altprofil, leere Config, bereits
migriert), Theme-Registry (jedes Theme vollständig, Akzent gesetzt), Zoom-Begrenzung.

**Visuelle Prüfung vor der Windows-Abnahme.** Statt zu raten wird eine statische Vorschauseite
gerendert: `index.css` + `App.css` mit nachgebautem Shell-Markup und Beispieldaten, per
Headless-Chromium in mehreren Fenstergrößen fotografiert. Damit lassen sich Scrollverhalten,
Sidebar-Dichte und Kontraste hier prüfen, ohne auf einen Windows-Build zu warten. Die
Kontrastwerte werden **gerechnet**, nicht geschätzt.

**Windows-Abnahme** — neuer M8-Block in `docs/e2e-checklist.md`:

1. Kein Fenster-Scrollbalken bei irgendeiner Fenstergröße; Sidebar und Panel scrollen intern.
2. Fenster sehr schmal und sehr breit ziehen — nichts überlappt, kein Textüberlauf.
3. Leere Gruppen sind unsichtbar; das ⋮ erscheint bei Hover **und** per Tab.
4. Jedes Theme wechselt Terminalfarben **und** App-Akzent; die Wahl überlebt einen Neustart.
5. Schriftgröße ändern → tmux-Geometrie stimmt weiter (kein abgeschnittener Text, kein Umbruch
   an falscher Stelle); `Strg +/-/0` wirken sofort.
6. Zweites Profil anlegen, verbinden, zwischen Profilen wechseln; Passwörter beider Profile
   liegen getrennt im Anmeldedaten-Speicher.
7. Eine `config.json` aus M7 (ohne `profiles`) startet ohne Fehler und erscheint als Profil
   „default" — mit weiterhin funktionierendem gespeichertem Passwort.
8. Einstellungen über Zahnrad und `Strg+,`; jede Änderung wirkt ohne Neustart.

---

## Bewusst außerhalb des Scopes

- **Heller App-Modus** — bewusst abgewählt: doppelte Kontrastpflege ohne echten Nutzen für ein
  Terminal-Werkzeug.
- **WebGL-Renderer** — siehe Abschnitt 3, ohne gemessenen Bedarf.
- **Fensterposition merken** — bräuchte `tauri-plugin-window-state`; separat entscheidbar.
- **Session umbenennen**, SFTP-Panel (M6), Key-Auth-Abnahme — unverändert offen.
- **Die offene M7-Frage** (nimmt `/effort` sein Argument inline?) bleibt bestehen und wird bei
  der nächsten Windows-Abnahme mitgeprüft.
