import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { clearToasts, dismissToast, pushToast, toasts } from './toasts';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  clearToasts();
  vi.useRealTimers();
});

describe('toasts', () => {
  it('closes success and info after 5 s, warnings after 8 s, and keeps errors', () => {
    pushToast({ tone: 'success', message: 'a' });
    pushToast({ tone: 'warning', message: 'b' });
    pushToast({ tone: 'error', message: 'c' });
    vi.advanceTimersByTime(5000);
    expect(toasts.value.map((t) => t.message)).toEqual(['b', 'c']);
    vi.advanceTimersByTime(3000);
    expect(toasts.value.map((t) => t.message)).toEqual(['c']);
    vi.advanceTimersByTime(60_000);
    expect(toasts.value).toHaveLength(1);
  });

  it('shows at most four', () => {
    for (let i = 0; i < 6; i += 1) pushToast({ tone: 'error', message: `m${i}` });
    expect(toasts.value.map((t) => t.message)).toEqual(['m2', 'm3', 'm4', 'm5']);
  });

  it('dismisses by id', () => {
    const id = pushToast({ tone: 'error', message: 'x' });
    dismissToast(id);
    expect(toasts.value).toHaveLength(0);
  });
});
