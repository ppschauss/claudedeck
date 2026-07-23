/**
 * Toast-Anzeige (Task 6): `start_project`/`kill_session`-Fehler (z.B. `tmuxMissing`) landen
 * hier statt (nur) als Inline-Fehlertext in der Sidebar. Wird einmal in `App.tsx` gemountet,
 * unabhängig vom Connect-Status.
 */
import { useToastStore } from "../stores/toastStore";

export function ToastHost() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);

  if (toasts.length === 0) return null;

  return (
    <div className="toast-host">
      {toasts.map((t) => (
        <div key={t.id} className="toast">
          <span>{t.message}</span>
          <button type="button" onClick={() => dismiss(t.id)} aria-label="Hinweis schließen">
            ×
          </button>
        </div>
      ))}
    </div>
  );
}
