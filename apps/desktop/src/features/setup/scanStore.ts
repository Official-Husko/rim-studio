import { signal } from '@preact/signals';
import type { ApiError, LibraryScanResult } from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { scanLibrary } from './api';
import { loadSources } from './sourcesStore';

export const scanResult = signal<LibraryScanResult | undefined>(undefined);
/** Which kind of scan is running; undefined when none. */
export const scanning = signal<'scan' | 'full' | undefined>(undefined);
export const scanError = signal<ApiError | undefined>(undefined);

/** Scan the library; a full scan ignores the cache. Source counts are reloaded afterwards. */
export async function runScan(full: boolean): Promise<void> {
  if (scanning.value) return;
  scanning.value = full ? 'full' : 'scan';
  try {
    scanResult.value = await scanLibrary(full);
    scanError.value = undefined;
    await loadSources();
  } catch (thrown) {
    scanError.value = normalizeError(thrown);
  } finally {
    scanning.value = undefined;
  }
}

/** Forget everything (tests, session change). */
export function resetScan(): void {
  scanResult.value = undefined;
  scanning.value = undefined;
  scanError.value = undefined;
}
