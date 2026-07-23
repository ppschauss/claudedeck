/**
 * Schlichter globaler Toast-Store (Task 6): `start_project`/`kill_session`-Fehler laufen als
 * Toast statt (nur) als Inline-Fehlertext. Kein pure-Reducer-Unterbau nötig — anders als
 * `sessionStore`/`connectionStore` ist hier der Seiteneffekt (Auto-Dismiss nach 5s) selbst der
 * ganze Zweck, es gibt keine sinnvoll isolierbare reine Entscheidungslogik dahinter.
 */
import { create } from "zustand";

export interface Toast {
  id: string;
  message: string;
}

const AUTO_DISMISS_MS = 5000;

interface ToastStore {
  toasts: Toast[];
  push: (message: string) => void;
  dismiss: (id: string) => void;
}

let counter = 0;

export const useToastStore = create<ToastStore>((set) => ({
  toasts: [],
  push: (message) => {
    const id = `t${++counter}-${Date.now()}`;
    set((s) => ({ toasts: [...s.toasts, { id, message }] }));
    setTimeout(() => {
      set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
    }, AUTO_DISMISS_MS);
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));
