import { signal } from '@preact/signals';

export type ToastTone = 'info' | 'success' | 'warning' | 'error';

export interface ToastItem {
  id: number;
  tone: ToastTone;
  title?: string;
  message: string;
  actionLabel?: string;
  onAction?: () => void;
}

export type ToastInput = Omit<ToastItem, 'id'>;

const MAX_VISIBLE = 4;
const AUTO_DISMISS_MS: Record<ToastTone, number | undefined> = {
  info: 5000,
  success: 5000,
  warning: 8000,
  error: undefined,
};

export const toasts = signal<readonly ToastItem[]>([]);
let nextId = 0;
const timers = new Map<number, ReturnType<typeof setTimeout>>();

/** Close one toast. */
export function dismissToast(id: number): void {
  clearTimeout(timers.get(id));
  timers.delete(id);
  toasts.value = toasts.value.filter((t) => t.id !== id);
}

/** Show a toast. Success and info close after 5 s, warnings after 8 s, errors stay until closed; four at most. */
export function pushToast(input: ToastInput): number {
  nextId += 1;
  const item: ToastItem = { ...input, id: nextId };
  const next = [...toasts.value, item];
  while (next.length > MAX_VISIBLE) {
    const dropped = next.shift();
    if (dropped) {
      clearTimeout(timers.get(dropped.id));
      timers.delete(dropped.id);
    }
  }
  toasts.value = next;
  const after = AUTO_DISMISS_MS[input.tone];
  if (after !== undefined)
    timers.set(
      item.id,
      setTimeout(() => dismissToast(item.id), after),
    );
  return item.id;
}

/** Remove everything (tests). */
export function clearToasts(): void {
  for (const timer of timers.values()) clearTimeout(timer);
  timers.clear();
  toasts.value = [];
}
