/**
 * Bestätigungsdialog bei `ApiError.kind === "hostkeyUnknown"` (Erstkontakt mit einem Host).
 * "Abbrechen" ist per Vorgabe (Global Constraint / Task-5-Auftrag) der autoFocus- UND
 * Default-Button: er sitzt als `type="submit"` im umschließenden `<form>`, sodass Enter ihn
 * auslöst, ohne dass versehentliches Bestätigen per Tastatur möglich ist — Vertrauen in einen
 * neuen Host-Key erfordert einen bewussten Klick auf "Vertrauen & verbinden".
 */
interface HostKeyDialogProps {
  fingerprint: string;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}

export function HostKeyDialog({ fingerprint, busy, onCancel, onConfirm }: HostKeyDialogProps) {
  return (
    <div className="dialog-backdrop">
      <form
        className="dialog"
        onSubmit={(e) => {
          e.preventDefault();
          onCancel();
        }}
      >
        <h2>Unbekannter Host-Key</h2>
        <p>
          Dieser Server wurde noch nie akzeptiert. Prüfe den Fingerprint (z.B. gegen eine
          bekannte Ausgabe auf dem Server selbst), bevor du fortfährst:
        </p>
        <p className="fingerprint">
          <code>{fingerprint}</code>
        </p>
        <div className="dialog-actions">
          <button type="submit" autoFocus disabled={busy}>
            Abbrechen
          </button>
          <button type="button" onClick={onConfirm} disabled={busy}>
            Vertrauen &amp; verbinden
          </button>
        </div>
      </form>
    </div>
  );
}
