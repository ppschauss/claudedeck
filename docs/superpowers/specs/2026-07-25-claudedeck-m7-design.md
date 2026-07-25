# ClaudeDeck M7 — UTF-8/DE-Layout, Session-Sortierung & -Suche, Befehls-Panel, Model/Effort

## Context

ClaudeDeck (Tauri v2 · Rust/russh · React · xterm.js) verwaltet Claude-Code-tmux-Sessions auf
dem Homelab-Server. M0–M5 sind fertig. Im Windows-Build sind vier Lücken aufgefallen bzw.
gewünscht:

1. **Falsche Zeichen.** Ausgabe vom Server zeigt kaputte Umlaute und Rahmenzeichen; bei der
   Eingabe fehlen Umlaute/ß sowie die AltGr-Zeichen `@ { } [ ] \ | ~`. Ohne das ist die App für
   einen deutschen Nutzer schlicht nicht bedienbar.
2. **Session-Liste unsortiert und ungefiltert** — mit wachsender Zahl paralleler Sessions wird
   die Sidebar unübersichtlich.
3. **Keine Übersicht über verfügbare Befehle.** Skills, Agents und Connectors liegen auf dem
   Server; welche es gibt, weiß man nur aus dem Kopf.
4. **Model und Arbeitsstärke nicht steuerbar** — `--model`/`--effort` müssten heute per Hand
   getippt werden.

Ergebnis: bedienbare Tastatur- und Zeichenbehandlung, eine sortier- und durchsuchbare
Session-Liste, ein rechtes ausklappbares Befehls-Panel (Akkordeon nach Skill/Agent/Connector mit
eigener Suche), und Regler für Model/Effort.

**Zwei unabhängige Ursachen für Punkt 1** — der Befund, auf dem Abschnitt 1 aufbaut:
`ssh/pty.rs:53` fordert das PTY **ohne jede Env-Variable** an und `cmd_attach` ruft `tmux attach`
ohne `-u`. In einer `C`-Locale (der Kommentar in `tmux/commands.rs:21` belegt sie empirisch)
rendert tmux keine Rahmenzeichen und Readline verstümmelt 8-Bit-Eingaben — das erklärt beide
Umlaut-Symptome. Die AltGr-Zeichen sind dagegen reines ASCII, können also *nicht* an der Locale
liegen: das ist Tastatur-Event-Handling im WebView2.

---

## Abschnitt 1 — UTF-8 und deutsches Layout

### 1a Remote-Locale (Ausgabe + Umlaut-Eingabe)

`crates/claudedeck-core/src/tmux/commands.rs` ist laut eigenem Kopfkommentar „die einzige Stelle,
die Shell-Strings zusammensetzt" und bereits durchgehend unit-getestet — der Fix gehört dorthin,
nicht in `pty.rs` (`set_env` scheitert an sshds `AcceptEnv`).

- Neue Konstante `LOCALE_PREFIX = "LANG=C.UTF-8 LC_ALL=C.UTF-8"`. `C.UTF-8` statt `de_DE.UTF-8`,
  weil es auf glibc/musl ohne `locale-gen` immer existiert.
- `cmd_attach`: Prefix + `tmux -u attach` (`-u` erzwingt UTF-8 unabhängig von der
  Locale-Erkennung).
- `cmd_new_detached`: Prefix voranstellen.
- `cmd_list_sessions` / `cmd_list_panes` / `cmd_pane_cwd`: Prefix voranstellen — Session-Namen und
  Pfade können Umlaute enthalten.
- Bestehende Tests in derselben Datei (`attach_nutzt_exaktes_target`,
  `new_detached_quotet_alles_und_nutzt_a_d`, `list_sessions_nutzt_ascii_pipe_…`) auf die neuen
  Strings anpassen.

**`FIELD_SEP` nicht anfassen.** Der Kommentar bei `commands.rs:21` begründet das ASCII-`|` mit
locale-unabhängiger Sicherheit. Unter UTF-8 wäre `␞` (U+241E) nun möglich — aber der Parser
funktioniert, und ein Wechsel würde `tmux/parser.rs` mitreißen. Ausdrücklich außerhalb des Scopes.

### 1b AltGr (`@ { } [ ] \ | ~`)

xterm 6.0 hat die Logik bereits (`_isThirdLevelShift`,
`node_modules/@xterm/xterm/src/browser/CoreBrowserTerminal.ts:1105-1117`), aber sie hängt an
`isWindows` aus dem **deprecated** `navigator.platform`
(`src/common/Platform.ts:41`). Darauf nicht wetten — eigener Handler, unabhängig von der
Plattformerkennung.

- **Neu `src/lib/keyboard.ts`** — pure Funktion im Hausstil von `badges.ts`/`sessionSwitch.ts`
  (ohne DOM testbar, wichtig weil real nur über GitHub-Actions-Builds testbar):

  ```ts
  export function altGraphChar(ev: KeyboardEvent): string | null
  ```
  Liefert `ev.key`, wenn (`ev.getModifierState("AltGraph")` **oder**
  `ev.ctrlKey && ev.altKey && !ev.metaKey`) und `ev.key.length === 1`; sonst `null`.
  `ev.key === "Dead"` ergibt immer `null`, damit die Akzenttasten (´ ` ^) weiter komponieren.

- **`src/lib/termPool.ts`** — in `ensure()` nach `term.open(el)`:
  `term.attachCustomKeyEventHandler`, der bei einem Treffer `ev.preventDefault()` ruft, das
  Zeichen selbst über den bestehenden `onData`-Pfad (`encoder.encode(...)`) sendet und `false`
  zurückgibt. **Das `preventDefault` ist zwingend** — ohne es tippt der Browser das Zeichen
  zusätzlich in xterms versteckte Textarea und es käme doppelt an.

- **`src/components/TerminalHost.tsx:67`** — der globale Strg+F-Handler prüft nur `ctrlKey`; mit
  AltGr (= Strg+Alt) öffnet AltGr+F ungewollt die Suche. Bedingung um `&& !e.altKey` erweitern.

- **Tests** `src/lib/__tests__/keyboard.test.ts`: AltGraph-Modifier, Strg+Alt-Fallback,
  `Dead`-Durchlass, mehrzeichige Keys (`ArrowLeft`) unangetastet, echtes Strg+C (`altKey:false`)
  bleibt `null`.

---

## Abschnitt 2 — Session-Sortierung und -Suche

Ein Suchfeld + ein Sortier-Dropdown oben in der Sidebar, wirken auf alle drei Gruppen
(Angehängt/Läuft/Startbar); die Gruppierung bleibt erhalten.

- **Neu `src/lib/sessionFilter.ts`** — pure Funktionen, wieder Hausstil:
  - `matchesQuery(name: string, query: string): boolean` — case-insensitive Substring, getrimmt;
    leere Query matcht alles.
  - `sortByKey<T>(items: T[], key: SortKey, meta: (t: T) => SortMeta): T[]` mit
    `SortKey = "name" | "created" | "lastActive"`.
  - `SortMeta = { name: string; createdAt: number | null; lastOutputAt: number | null }`.
  - Sortierung ist **stabil** und bei fehlendem Feld auf `name` zurückfallend — `lastActive`
    kennt nur für angehängte Sessions einen echten Wert (`activity.lastOutputAt`), für die
    übrigen wird `session_created` genutzt. Das ist bewusst so und gehört in den Doc-Kommentar,
    damit die Sortierung nicht als exakter „letzter Zugriff" missverstanden wird.
- **`src/stores/sessionStore.ts`** — `query: string` und `sortBy: SortKey` plus Actions
  `queryChanged`, `sortChanged`. Reine Zustandsübergänge, kein IPC (wie der bestehende Store).
- **`src/components/Sidebar.tsx`** — Kopfzeile mit `<input type="search">` und `<select>`; die
  vorhandenen `useMemo`s für `attached`/`notAttached`/`startable` durch Filter+Sort führen.
  Gruppen mit 0 Treffern zeigen weiterhin `–` (bestehendes `sidebar-empty`).
- **Tests** `src/lib/__tests__/sessionFilter.test.ts`; Store-Actions in der bestehenden
  `src/stores/__tests__/sessionStore.test.ts` ergänzen.

---

## Abschnitt 3 — Befehls-Panel rechts

Layout wird `[Sessions | Terminal | Befehle]`; das rechte Panel klappt per Button und
Tastenkürzel ein/aus. Klick auf einen Eintrag **fügt `/name ` ins aktive Terminal ein und sendet
kein Enter** — der Nutzer tippt Argumente und drückt selbst Return.

### Discovery (Rust, testbar)

Zwei Quellen, beide über die bestehende gemultiplexte Verbindung
(`ssh/connection.rs:138 exec_capture`):

1. **Skills / Agents / Slash-Commands** — ein `exec_capture` mit einem `find`, das Kandidaten mit
   Trennmarken ausgibt (`===F:<pfad>` + die ersten ~2 KB je Datei, nur das Frontmatter wird
   gebraucht):
   - global: `~/.claude/skills/*/SKILL.md`, `~/.claude/agents/*.md`, `~/.claude/commands/*.md`
   - Plugin-Skills: `~/.claude/plugins/cache/*/*/skills/*/SKILL.md`
   - projektlokal: `<cwd der aktiven Session>/.claude/{skills,agents,commands}`
2. **Connectors** — separater `claude mcp list`. Verifiziert: liefert Name, URL **und
   Verbindungsstatus** (`√ Connected` / `! Needs authentication`) und ist damit besser als
   `~/.claude.json` zu parsen — dessen 51 KB enthalten außerdem History und Tokens, die hier
   nichts zu suchen haben.

- **Neu `crates/claudedeck-core/src/catalog/`** mit `mod.rs`, `parser.rs`, `commands.rs`:
  - `commands.rs` baut die Shell-Strings (über `shell_quote` aus `tmux/commands.rs`, damit
    Quoting an einer Stelle bleibt).
  - `parser.rs`: `parse_catalog(&str) -> Vec<CommandEntry>` und `parse_mcp_list(&str) ->
    Vec<Connector>` — reine Funktionen, mit Fixture-Strings unit-getestet, genau wie
    `tmux/parser.rs`. YAML-Frontmatter: nur `name` und `description` werden gelesen, per
    Zeilenscan zwischen den `---`-Markern (keine YAML-Dependency für zwei Felder). Fehlendes
    Frontmatter → Eintrag mit Dateinamen als Name und leerer Beschreibung, statt ihn zu verlieren.
  - `CommandEntry { kind: Skill|Agent|Command, name, description, scope: Global|Project }`.

### IPC

- **Neu `src-tauri/src/commands/catalog.rs`** mit `#[tauri::command] list_commands(session_id:
  Option<String>) -> Result<Catalog, ApiError>`. Eigene Datei, weil `sessions.rs` mit ~660 Zeilen
  schon groß ist. Registrierung in `commands/mod.rs` und der `invoke_handler`-Liste in `lib.rs`.
- Projektpfad der aktiven Session über das vorhandene `cmd_pane_cwd`.

### Frontend

- **Neu `src/components/CommandPanel.tsx`** — Akkordeon mit vier Gruppen (Skills, Agents,
  Commands, Connectors), projektlokale Einträge mit `●`-Marker, eigenes Suchfeld darüber.
  Aufklapp-Zustand pro Gruppe lokal (`useState`).
- **Neu `src/lib/catalogFilter.ts`** — pure `filterCatalog(entries, query)`; nutzt `matchesQuery`
  aus `sessionFilter.ts` wieder (Suche über Name **und** Beschreibung). Tests dazu.
- **Neu `src/stores/catalogStore.ts`** — `entries`, `loading`, `error`, `query`, gecacht pro
  Projektpfad; Reload bei Sessionwechsel und über einen Refresh-Button.
- **Einfügen** über das bestehende `writeSession(sessionId, bytes)` aus `src/lib/ipc.ts` — kein
  neuer IPC-Weg. Ohne aktive Session ist der Klick deaktiviert (mit Tooltip), nicht still
  wirkungslos.
- **`src/App.tsx` / `src/App.css`** — dritte Spalte im `app-body`, Toggle-Button und Kürzel.
  Nach dem Ein-/Ausklappen muss `termPool.fit(activeSessionId)` laufen, damit das Terminal die
  neue Breite bekommt — der `ResizeObserver` in `TerminalHost.tsx` erledigt das bereits, sofern
  die Breitenänderung am Host-`<div>` ankommt; das ist beim Testen zu prüfen.
- **`src/lib/ipc.ts`** — Typen + Wrapper für `list_commands`.

---

## Abschnitt 4 — Model- und Effort-Regler

Verifiziert über `claude --help` auf dem Server (Claude Code **2.1.220**):

- `--model <model>` — Alias (`fable`, `opus`, `sonnet`) oder voller Name (`claude-fable-5`)
- `--effort <level>` — `low`, `medium`, `high`, `xhigh`, `max`

Wirkt auf **beides**: laufende Session *und* Vorgabe für neue.

- **Laufende Session** — `/model <x>` bzw. `/effort <y>` per `writeSession` einfügen (gleicher
  Mechanismus wie Abschnitt 3, ohne Enter).
  **Offene Verifikation:** `/model`, `/effort` und `/fast` sind als Slash-Kommandos im Bundle
  belegt, aber **ob sie ein Argument inline annehmen** (`/effort high`) oder einen interaktiven
  Picker öffnen, ist ungeprüft. Erster Schritt dieses Abschnitts ist ein manueller Test in einer
  echten Session. Öffnet sich ein Picker, ist der Fallback: nur das nackte `/effort` einfügen und
  den Nutzer im Picker wählen lassen — Pfeiltasten zu simulieren wäre zu fragil.
- **Neue Sessions** — `Config` in `crates/claudedeck-core/src/config.rs` erhält
  `defaults: SessionDefaults { model: Option<String>, effort: Option<String> }` (mit
  `#[serde(default)]`, damit bestehende `config.json` weiter laden — der Test
  `partial_json_with_defaults` deckt das Muster schon ab). `cmd_new_detached` baut daraus
  `claude --model <m> --effort <e>`; beide Felder optional, `None` lässt das Flag weg.
- **Model-Liste nicht hart kodieren.** Sie kommt aus der Config (`available_models: Vec<String>`)
  mit den Aliassen `opus`/`sonnet`/`haiku`/`fable` als Default. So braucht ein neues Modell keine
  Code-Änderung — hart kodierte Model-IDs veralten sonst zuverlässig.
- **UI** — kompakte Zeile im Kopf des rechten Panels (session-bezogener Kontext): `<select>` für
  das Model, 5-stufiger `<input type="range">` für den Effort mit Textlabel. Änderung schreibt
  über `setConfig` die Vorgabe **und** fügt das Slash-Kommando in die aktive Session ein.
- **Effort-Stufen als pure Abbildung** in `src/lib/effort.ts`
  (`EFFORT_LEVELS = ["low","medium","high","xhigh","max"]`, Index ↔ Level), damit der Regler
  ohne DOM testbar bleibt.

---

## Verifikation

**Automatisiert (auf dem Linux-Host):**

```bash
./dev.sh cargo test        # tmux/commands.rs + catalog/parser.rs (kein webkit2gtk nötig)
npx vitest run             # keyboard.ts, sessionFilter.ts, catalogFilter.ts, effort.ts, Stores
npm run build              # tsc -b + vite build
npx oxlint
```

**Manuell auf dem Server (lesend, vor der Implementierung von Abschnitt 4):**

```bash
./dev.sh cargo run --example spike -- <host> <user> exec \
  "LANG=C.UTF-8 LC_ALL=C.UTF-8 tmux -u ls"      # Locale-Prefix greift
ssh <host> 'claude mcp list'                     # Parser-Fixture für parse_mcp_list gewinnen
ssh <host> 'ls ~/.claude/skills ~/.claude/agents ~/.claude/commands'
```

**Windows-Abnahme** — `docs/e2e-checklist.md` um diese Punkte erweitern (nur dort real prüfbar,
Build über GitHub Actions `build.yml`):

1. Umlaute in der Ausgabe korrekt; Claude-Codes Rahmenzeichen sauber gezeichnet.
2. `ä ö ü ß` getippt kommen im Terminal an.
3. AltGr+Q/7/8/9/0/ß/< ergeben `@ { [ ] } \ |`; AltGr+E ergibt `€`.
4. Tote Tasten: `` ` ``+Leertaste ergibt `` ` ``; `^`+`a` ergibt `^a`.
5. AltGr+F öffnet **nicht** die Suche; Strg+F öffnet sie weiter.
6. Sidebar-Suche filtert alle drei Gruppen; jede Sortierung ändert die Reihenfolge sichtbar.
7. Befehls-Panel: ein-/ausklappen passt die Terminalbreite an; Klick fügt `/name ` ohne Enter
   ein; projektlokale Einträge wechseln beim Sessionwechsel.
8. Model/Effort: Regler fügt das Slash-Kommando ein; nach App-Neustart startet eine neue Session
   mit den gemerkten Flags (`claude --model … --effort …` im Prozessbaum prüfbar).

---

## Bewusst außerhalb des Scopes

- **SFTP-Panel (M6)** — unberührt.
- **`FIELD_SEP`-Wechsel** auf ein Unicode-Trennzeichen (Begründung in Abschnitt 1a).
- **`/fast`-Umschalter** — im Bundle vorhanden, aber nicht angefragt.
- **Key-Auth** — weiterhin offen aus M5.
