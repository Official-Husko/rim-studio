import { signal } from '@preact/signals';
import { probeFolder } from './api';
import { CE_WORKSHOP_ID, joinPath } from './model';

/** What is known about Combat Extended on this machine. */
export type CeState =
  | { status: 'unknown' }
  | { status: 'checking' }
  | { status: 'found'; path: string; mods: number }
  | { status: 'missing'; path: string };

export const ce = signal<CeState>({ status: 'unknown' });

/**
 * Look for the Combat Extended workshop item inside the detected workshop folder. The backend has
 * no "is Combat Extended in the library" command yet, so the folder of its Steam item is probed
 * with the same command that checks folders before they are added.
 */
export async function checkCombatExtended(workshopDir: string | undefined): Promise<void> {
  if (!workshopDir) {
    ce.value = { status: 'unknown' };
    return;
  }
  const path = joinPath(workshopDir, CE_WORKSHOP_ID);
  ce.value = { status: 'checking' };
  try {
    const probe = await probeFolder(path);
    ce.value =
      probe.kind === 'single-mod' || probe.kind === 'mods-root'
        ? { status: 'found', path, mods: probe.modCount }
        : { status: 'missing', path };
  } catch {
    ce.value = { status: 'missing', path };
  }
}

/** Forget everything (tests, session change). */
export function resetCe(): void {
  ce.value = { status: 'unknown' };
}
