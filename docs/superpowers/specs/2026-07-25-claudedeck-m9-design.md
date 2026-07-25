# ClaudeDeck M9 — Projektfilter, echte Sortierung, Ablage mit Vorschau und Download

## Context

Nach dem M8-Build sind beim Benutzen drei Dinge aufgefallen:

1. **„Startbar" ist überflutet.** Der Scan listet *jedes* Unterverzeichnis der `scan_paths` —
   auf dem Server sind das **88 Ordner in `/mnt/cache/appdata`**, von denen nur **9** überhaupt
   ein Projekt sind. Die übrigen 79 sind Docker-Datenverzeichnisse (Kaizoku, FileBot, Immich …),
   die dort nichts zu suchen haben.
2. **Die Sortierung wirkt in „Startbar" nicht.** Projekte tragen bisher keinen Zeitstempel,
   deshalb fallen „Zuletzt aktiv" und „Startzeit" dort still auf Namenssortierung zurück — die
   Auswahl ändert sichtbar nichts.
3. **Dateien, die in einer Session entstehen, sind unerreichbar.** Erzeugt Claude ein Bild, einen
   Bericht oder ein Paket, sieht man im Terminal höchstens den Dateinamen. Es fehlt eine Ablage,
   die solche Dateien anzeigt, Bilder vorschaut und beides herunterladbar macht.

Ziel: eine Sidebar, die nur zeigt, woran man wirklich arbeitet und sich sinnvoll sortieren lässt —
und ein Weg, an das heranzukommen, was in einer Session entsteht.

### Was schon da ist und wiederverwendet wird

- **`russh-sftp = "2.1"`** steht seit M0 in `crates/claudedeck-core/Cargo.toml`, wird aber
  **nirgends benutzt** — das ist das nie gebaute M6. Die Ablage ist im Kern genau dieses Panel.
- **`dirs = "6"`** (beide Crates) liefert `download_dir()` — der Download braucht damit **kein**
  `tauri-plugin-dialog`/`-fs` und **keine** Änderung an `capabilities/default.json`.
- **`data-encoding = "2"`** (src-tauri) macht die Bildvorschau über denselben Base64-Weg, den
  `OutputChunk` für PTY-Daten schon geht.
- **`SshConnection`** (`ssh/connection.rs:101`) hat `exec_capture` und `open_pty`; SFTP kommt als
  dritte Methode daneben, über dieselbe gemultiplexte Verbindung.
- **`sessionFilter.ts`** sortiert bereits generisch über `SortMeta` — es fehlt nur der
  Zeitstempel auf der Projektseite, nicht die Sortierlogik.

---

## Abschnitt 1 — Nur echte Projekte, mit Zeitstempel

Beide Probleme hängen an derselben Stelle: `cmd_scan_projects` in `tmux/commands.rs`.

**Filtern nach Projekt-Merkmal.** Aufgenommen wird ein Ordner nur, wenn er eines von
`.git`, `.claude` oder `CLAUDE.md` enthält. Auf dem Server: 88 → 9. Die Merkmalsliste kommt aus
der Config (`project_markers`, Vorgabe diese drei) und ist im Einstellungen-Dialog änderbar —
wer andere Konventionen nutzt, soll nicht auf einen Rebuild warten müssen.

**Zeitstempel mitlesen.** Das Kommando gibt je Projekt `<mtime>\t<pfad>` aus. Als Zeit dient die
**neueste Änderungszeit unter den Einträgen der obersten Ebene** (`find "$d" -maxdepth 1
-printf '%T@\n' | sort -rn | head -1`), nicht die des Ordners selbst: letztere ändert sich nur,
wenn Dateien hinzukommen oder verschwinden, nicht wenn eine bearbeitet wird. Damit steht bei
„Zuletzt aktiv" das zuletzt angefasste Projekt oben.

**Änderungen:**
- `tmux/commands.rs`: `cmd_scan_projects(paths, markers)` baut Filter und Zeitausgabe; alle Werte
  weiter durch `shell_quote`.
- **Neu** `tmux/parser.rs::parse_projects(&str) -> Vec<ProjectEntry>` — das Zerlegen passiert
  heute inline in `sessions.rs:377ff`. Als pure Funktion mit Fixtures testbar, wie die übrigen
  Parser dort. Fehlerhafte Zeilen werden übersprungen statt die Liste zu kippen.
- `Project` (IPC) bekommt `modified: number` (Unix-Sekunden); `Sidebar.tsx` reicht es als
  `createdAt` in die vorhandene `SortMeta` — an `sessionFilter.ts` ändert sich **nichts**.
- `config.rs`: `project_markers: Vec<String>`.

---

## Abschnitt 2 — Ablage: Dateibrowser mit Vorschau und Download

Das rechte Panel bekommt zwei Reiter: **Befehle** und **Ablage**. Damit bleiben `Strg+B`, der
Umschalter und das vorhandene CSS-Gerüst; es entsteht keine vierte Spalte.

### Kern (Rust)

**Neu `crates/claudedeck-core/src/sftp/mod.rs`:**
- `SshConnection::open_sftp()` — Session-Kanal, `request_subsystem("sftp")`, daraus eine
  `russh_sftp::client::SftpSession`.
- `list_dir(path) -> Vec<RemoteEntry>` mit `name`, `is_dir`, `size`, `modified`.
- `read_file(path, max_bytes) -> Vec<u8>` — mit Obergrenze, damit eine versehentlich angeklickte
  Riesendatei nicht den Speicher und die IPC-Brücke flutet.

SFTP-Pfade gehen **nicht** durch die Shell, also entfällt hier das Quoting-Thema — dafür werden
Pfade nie aus Frontend-Eingaben zusammengesetzt, sondern immer aus dem, was `list_dir` geliefert
hat.

**Neu `src-tauri/src/commands/files.rs`** (eigene Datei, wie `catalog.rs` — `sessions.rs` ist mit
~680 Zeilen groß genug):
- `list_directory(path) -> Vec<RemoteEntry>`
- `preview_file(path) -> { mime, dataB64 }` — nur für Bilder, hart gedeckelt (**8 MB**);
  darüber liefert das Backend eine Absage, und die UI bietet nur den Download an.
- `download_file(path) -> String` — lädt per SFTP und schreibt nach `dirs::download_dir()`,
  liefert den lokalen Pfad zurück. Existiert der Name schon, wird ` (2)`, ` (3)` … angehängt,
  statt eine vorhandene Datei zu überschreiben.

### Oberfläche

- **Neu `src/lib/fileKind.ts`** — pure: Endung → Art (`image` | `text` | `archive` | `other`)
  plus Symbol. Unit-getestet; hält die Fallunterscheidung aus dem JSX heraus.
- **Neu `src/stores/filesStore.ts`** — aktueller Pfad, Einträge, Ladezustand, Auswahl.
- **Neu `src/components/FilePanel.tsx`** — Pfadzeile mit „aufwärts", Ordner zum Hineingehen,
  Dateien mit Größe und Alter. **Vorgabe-Sortierung: neueste zuerst** — genau das, was man nach
  „Claude hat mir gerade ein Bild gebaut" sehen will, ohne extra Filter.
- **Bildvorschau** inline im Panel (Data-URL), darunter ein Download-Knopf. Andere Dateitypen
  zeigen nur den Download.
- Startpunkt ist das Arbeitsverzeichnis der aktiven Session (`SessionInfo.cwd`, dieselbe Quelle,
  die das Befehls-Panel für die projektlokalen Einträge nutzt). Ohne aktive Session bleibt das
  Panel leer mit Hinweis.

### Bewusste Grenzen

- **Nur Lesen und Herunterladen.** Kein Upload, kein Umbenennen, kein Löschen — ein Dateimanager
  mit Schreibrechten auf einen Server, auf dem `root` arbeitet, braucht eine Sicherheitsdebatte,
  die dieser Auftrag nicht führt.
- **Kein Auto-Refresh.** Ein Knopf zum Neuladen genügt; ein Beobachter über SFTP wäre Pollerei.

---

## Reihenfolge

1. **Abschnitt 1** — klein, behebt sofort die zwei sichtbaren Ärgernisse.
2. **Abschnitt 2, Kern** — SFTP im core-Crate, gegen den echten Server erprobbar (`dev.sh`).
3. **Abschnitt 2, Oberfläche** — Panel, Vorschau, Download.

---

## Verifikation

**Automatisiert — an `ci.yml` ausgerichtet** (fmt und clippy gehören dazu, das war die Lücke in
M7):

```bash
./dev.sh cargo fmt --all --check
./dev.sh cargo clippy -p claudedeck-core --all-targets -- -D warnings
./dev.sh cargo test -p claudedeck-core
./dev.sh cargo check -p app
node_modules/.bin/tsc -b tsconfig.json && node_modules/.bin/vitest run && node_modules/.bin/oxlint src
```

Neue Unit-Tests: `parse_projects` (gültige Zeile, fehlender Zeitstempel, kaputte Zeile, leere
Ausgabe), `cmd_scan_projects` (Merkmale und Quoting), `fileKind` (Endungen, Groß-/Kleinschreibung,
Datei ohne Endung), Namenskollision beim Download.

**Gegen den echten Server prüfbar, noch hier:**

```bash
# Filter: muss 9 statt 88 liefern
./dev.sh cargo run --example spike -- <host> <user> exec "<erzeugtes scan-Kommando>"
```

**Windows-Abnahme** — neuer M9-Block in `docs/e2e-checklist.md`:

1. „Startbar" zeigt nur noch echte Projekte; ein neu angelegtes Projekt mit `.git` erscheint nach
   dem Neuladen.
2. Sortierung „Zuletzt aktiv" ordnet die Projekte sichtbar um; das eben bearbeitete steht oben.
3. Ablage öffnet im Ordner der aktiven Session, Ordnernavigation und „aufwärts" funktionieren.
4. Ein von Claude erzeugtes PNG steht oben und zeigt eine Vorschau.
5. Download landet im Downloads-Ordner; zweimal herunterladen erzeugt „ (2)" statt zu
   überschreiben.
6. Eine Datei über 8 MB bietet nur Download, keine Vorschau — und stürzt nicht ab.
7. Sessionwechsel in ein anderes Projekt wechselt den Ordner der Ablage.
8. Ohne offene Session ist die Ablage leer mit Hinweis statt kaputt.

---

## Bewusst außerhalb des Scopes

- **Schreibender Dateizugriff** (Upload/Löschen/Umbenennen) — siehe oben.
- **Inline-Bilder im Terminal** (kitty/iTerm-Grafikprotokoll) — xterm.js kann das nicht; die
  Ablage ist der Weg dorthin.
- Offen aus M7/M8: die AltGr-Abnahme, ob `/effort` sein Argument inline nimmt, der Key-Auth-Pfad,
  und dass `notifications.silence_ms` vom `NotificationManager` weiterhin nicht gelesen wird.
