# ClaudeDeck

Schlanke Windows-Desktop-App zum Verwalten von Claude-Code-Sessions auf einem Homelab-Server:
verbindet sich per SSH, listet tmux-Sessions und öffnet sie als vollwertige Terminals (mehrere
parallel über eine gemultiplexte Verbindung). Ein SFTP-Seitenpanel für Up-/Downloads ist für
M6 geplant.

**Stack:** Tauri v2 · Rust (russh 0.62, russh-sftp) · React + TypeScript · xterm.js

## Status

M0–M5 abgeschlossen: verbinden, mehrere Sessions parallel, Badges/Notifications, Suche,
Auto-Reconnect. Offen: SFTP (M6), Windows-Abnahme, Key-Auth. Design und Pläne unter
[`docs/superpowers/`](docs/superpowers/).

## Benutzung

1. **Erste Verbindung:** Passwort eingeben, optional „im Windows-Anmeldedaten-Speicher
   merken" ankreuzen — bei bekanntem Host verbindet die App danach automatisch. Bei einem
   noch unbekannten Host-Key zeigt ein Dialog den Fingerprint zur Bestätigung.
2. **Session öffnen:** In der Sidebar auf einen laufenden Eintrag (●/○) klicken — das
   Terminal öffnet sich sofort, mehrere Sessions bleiben parallel im Hintergrund aktiv.
3. **Projekt starten:** Einen Eintrag unter „+ startbar" anklicken — legt eine neue
   `claude`-Session im jeweiligen Projektordner an und hängt sich sofort an.
4. **Scrollback durchsuchen:** Strg+F im aktiven Terminal öffnet die Suche
   (Enter/Shift+Enter für nächsten/vorherigen Treffer, Esc schließt).

Details und die manuelle Windows-Abnahme-Checkliste: [`docs/e2e-checklist.md`](docs/e2e-checklist.md).

## Entwicklung

Entwickelt wird auf einem Linux-Host ohne Rust-Toolchain — alle Cargo-Kommandos laufen im
Container:

```bash
./dev.sh cargo test              # Tests (nur claudedeck-core, kein webkit2gtk nötig)
./dev.sh cargo run --example spike -- <host> <user> exec "tmux ls"   # SSH-Spike
npx vitest run                   # Frontend-Tests
```

`secrets.env` (gitignored, chmod 600) stellt `SPIKE_SSH_PASSWORD` für den Spike bereit.

## Build

Windows-Builds entstehen ausschließlich in GitHub Actions (`build.yml`, windows-latest):
`.msi`-Installer + portable `claudedeck.exe` als Artifact `claudedeck-windows`.

## Installations-Warnung von Windows

Beim Ausführen meldet Windows „Der Herausgeber konnte nicht verifiziert werden" bzw.
SmartScreen blockt. **Das liegt nicht an fehlenden Angaben in der Datei, sondern daran, dass
sie nicht signiert ist** — die Metadaten unten ändern daran nichts.

Vollständig verschwindet die Warnung nur mit einem gekauften Code-Signing-Zertifikat, und selbst
dann sofort nur bei einem EV-Zertifikat (bei OV baut SmartScreen erst über Downloads Reputation
auf). Für den Eigengebrauch genügt ein selbst ausgestelltes Zertifikat, dem der eigene Rechner
vertraut:

```powershell
# 1) Einmalig ein Code-Signing-Zertifikat erzeugen. Alles unter Cert:\CurrentUser\,
#    deshalb genügt eine normale PowerShell ohne Administratorrechte.
$cert = New-SelfSignedCertificate `
  -Type CodeSigningCert `
  -Subject "CN=Patrick Schauss" `
  -CertStoreLocation Cert:\CurrentUser\My `
  -NotAfter (Get-Date).AddYears(5)

# 2) Fingerabdruck notieren — er kommt in tauri.conf.json bzw. in ein GitHub-Secret
$cert.Thumbprint

# 3) Dem Zertifikat auf diesem Rechner vertrauen
Export-Certificate -Cert $cert -FilePath "$env:TEMP\claudedeck.cer"
Import-Certificate -FilePath "$env:TEMP\claudedeck.cer" -CertStoreLocation Cert:\CurrentUser\Root
Import-Certificate -FilePath "$env:TEMP\claudedeck.cer" -CertStoreLocation Cert:\CurrentUser\TrustedPublisher

# 4) Eine vorhandene Datei damit signieren (oder den Fingerabdruck in
#    tauri.conf.json unter bundle.windows.certificateThumbprint eintragen)
Set-AuthenticodeSignature -FilePath .\claudedeck.exe -Certificate $cert `
  -TimestampServer "http://timestamp.digicert.com"
```

Danach zeigt Windows „Patrick Schauss" als Herausgeber statt „Unbekannt", und auf diesem Rechner
erscheint keine Warnung mehr. Auf fremden Rechnern bleibt sie — dort ist das Zertifikat nicht
hinterlegt, und das ist genau der Zweck der Übung.

### Automatisch signiert bauen

`build.yml` signiert bereits selbst, sobald zwei Repository-Secrets gesetzt sind. Fehlen sie,
baut der Workflow unverändert unsigniert weiter — ein fehlendes Zertifikat kippt den Build nicht.

Zertifikat aus Schritt 1 exportieren und als base64 ablegen:

```powershell
$pw = Read-Host -AsSecureString "Passwort für die .pfx"
Export-PfxCertificate -Cert "Cert:\CurrentUser\My\$($cert.Thumbprint)" `
  -FilePath "$env:TEMP\claudedeck.pfx" -Password $pw

# Base64 in die Zwischenablage — dieser Text kommt ins Secret
[Convert]::ToBase64String([IO.File]::ReadAllBytes("$env:TEMP\claudedeck.pfx")) | Set-Clipboard
Remove-Item "$env:TEMP\claudedeck.pfx"
```

Unter *Settings → Secrets and variables → Actions* anlegen:

| Secret | Inhalt |
| --- | --- |
| `WINDOWS_CERT_BASE64` | der base64-Text aus der Zwischenablage |
| `WINDOWS_CERT_PASSWORD` | das eben vergebene Passwort |

Der nächste Build importiert das Zertifikat, reicht seinen Fingerabdruck über `--config` an
`tauri build` weiter und prüft anschließend nach, dass `.exe` und `.msi` wirklich einen Signierer
tragen. Der Prüfschritt meldet dabei `UnknownError` statt `Valid` — der Runner kennt das selbst
ausgestellte Zertifikat nicht und misstraut der Kette. Signiert ist die Datei trotzdem; auf einem
Rechner, der das Zertifikat importiert hat (Schritt 3), steht `Valid`.

Die `.pfx` selbst gehört **nicht** ins Repository. Sie liegt nur im Secret, und der Workflow
löscht sie nach dem Import wieder vom Runner.

## Autor

Patrick Schauss · <info@patrickschauss.de> · [patrickschauss.de](https://patrickschauss.de)

Diese Angaben stehen auch in `src-tauri/tauri.conf.json` unter `bundle` und erscheinen dadurch
im Installer sowie in den Dateieigenschaften der `.exe`.
