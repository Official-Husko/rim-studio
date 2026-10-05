const KEY = 'rimstudio.wizard.prefix';

/** The def name prefix the user typed last time; empty when none or when storage is unavailable. */
export function readPrefix(): string {
  try {
    return localStorage.getItem(KEY) ?? '';
  } catch {
    return '';
  }
}

/** Remember the prefix for the next wizard. Failures are ignored: it is only a convenience. */
export function writePrefix(prefix: string): void {
  try {
    localStorage.setItem(KEY, prefix);
  } catch {
    // storage can be blocked or full; the wizard works without it
  }
}
