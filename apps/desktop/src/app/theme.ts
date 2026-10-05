import { signal } from '@preact/signals';

export type Density = 'compact' | 'comfortable' | 'roomy';

const KEY = 'rimstudio.density';
const VALID: readonly Density[] = ['compact', 'comfortable', 'roomy'];

function stored(): Density {
  try {
    const value = localStorage.getItem(KEY);
    if ((VALID as readonly (string | null)[]).includes(value)) return value as Density;
  } catch {
    /* storage can be blocked */
  }
  return 'comfortable';
}

/** The row and control density; applied as data-density on the root element. */
export const density = signal<Density>(stored());

/** Apply the theme attributes to the document. Dark is the only theme for now. */
export function applyTheme(root: HTMLElement = document.documentElement): void {
  root.setAttribute('data-theme', 'dark');
  root.setAttribute('data-density', density.value);
}

/** Change the density and remember it for this viewer. */
export function setDensity(next: Density): void {
  density.value = next;
  applyTheme();
  try {
    localStorage.setItem(KEY, next);
  } catch {
    /* a viewer convenience only */
  }
}
