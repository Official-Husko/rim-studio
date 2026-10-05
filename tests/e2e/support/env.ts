import { homedir } from 'node:os';
import { join } from 'node:path';

/** What the global setup exports to the workers (all values are strings). */
export interface E2eEnv {
  /** Base URL of the Vite dev server. */
  baseUrl: string;
  /** Base URL of the bridge. */
  bridgeUrl: string;
  /** Bridge token. */
  token: string;
  /** Temporary root of everything the run writes. */
  tmp: string;
  /** Folder for screenshots. */
  shots: string;
}

const KEY = 'RIMSTUDIO_E2E_ENV';

/** Reads the environment exported by the global setup. */
export function readEnv(): E2eEnv {
  const raw = process.env[KEY];
  if (!raw) throw new Error('The e2e stack is not running (global setup did not export its env).');
  return JSON.parse(raw) as E2eEnv;
}

/** Exports the environment for the worker processes. */
export function writeEnv(env: E2eEnv): void {
  process.env[KEY] = JSON.stringify(env);
}

/** The owner's real data, read only. */
export const REAL = {
  gameDir:
    process.env.RIMSTUDIO_GAME_DIR ?? join(homedir(), '.steam/steam/steamapps/common/RimWorld'),
  workshopDir:
    process.env.RIMSTUDIO_WORKSHOP_DIR ??
    join(homedir(), '.steam/steam/steamapps/workshop/content/294100'),
  ceDir:
    process.env.RIMSTUDIO_CE_DIR ??
    join(homedir(), '.steam/steam/steamapps/workshop/content/294100/2890901044'),
  customDir:
    process.env.RIMSTUDIO_CUSTOM_DIR ??
    '/run/media/pawbeans/project_drive/pawbeans/Projects/RimWorld Mods',
};
