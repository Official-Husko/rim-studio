import { signal } from '@preact/signals';
import type { ApiError, SettingsDto } from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { getSettings } from './api';

export const settings = signal<SettingsDto | undefined>(undefined);
export const settingsError = signal<ApiError | undefined>(undefined);

/** Read the settings for the read only summary. */
export async function loadSettings(): Promise<void> {
  try {
    settings.value = await getSettings();
    settingsError.value = undefined;
  } catch (thrown) {
    settingsError.value = normalizeError(thrown);
  }
}

/** Forget everything (tests, session change). */
export function resetSettings(): void {
  settings.value = undefined;
  settingsError.value = undefined;
}
