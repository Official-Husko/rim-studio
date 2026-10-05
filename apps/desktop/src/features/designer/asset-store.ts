import { signal } from '@preact/signals';
import type { ApiError, DefRowDto, DesignerAssetInfoResponse } from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { pickFile, type FileFilter } from '~/shared/platform';
import * as api from './asset-api';

/** What is known about one source file the draft names. */
export type FactState =
  | { status: 'loading' }
  | { status: 'ready'; info: DesignerAssetInfoResponse }
  | { status: 'failed'; error: ApiError };

/** The state of the search for vanilla sound definitions. */
export interface SoundSearch {
  query: string;
  rows: DefRowDto[];
  total: number;
  status: 'idle' | 'loading' | 'ready' | 'failed';
  error?: ApiError;
}

export interface AssetDeps {
  /** The id of the current project; relative paths are read against it. */
  projectId: () => string | undefined;
}

const IDLE_SEARCH: SoundSearch = { query: '', rows: [], total: 0, status: 'idle' };

/**
 * The facts of the files a draft imports and the search for vanilla sounds. The facts come from
 * `designer_asset_info`: the page never opens a file itself. A late answer of an older search never
 * replaces a newer one.
 */
export function createAssetStore(deps: AssetDeps) {
  const facts = signal<ReadonlyMap<string, FactState>>(new Map());
  const sounds = signal<SoundSearch>(IDLE_SEARCH);
  const pickError = signal<ApiError | undefined>(undefined);
  let searchSeq = 0;

  function put(path: string, state: FactState): void {
    const next = new Map(facts.peek());
    next.set(path, state);
    facts.value = next;
  }

  /** Read the facts of a file; a path that is already known is not read again unless forced. */
  async function load(path: string, force = false): Promise<void> {
    const known = facts.peek().get(path);
    if (!force && known && known.status !== 'failed') return;
    put(path, { status: 'loading' });
    try {
      put(path, { status: 'ready', info: await api.assetInfo(path, deps.projectId()) });
    } catch (thrown) {
      put(path, { status: 'failed', error: normalizeError(thrown) });
    }
  }

  /** Ask for a file with the system dialog (or the bridge browser), then read its facts. */
  async function choose(filters: FileFilter[]): Promise<string | null> {
    pickError.value = undefined;
    try {
      const path = await pickFile({ filters });
      if (path) await load(path, true);
      return path;
    } catch (thrown) {
      pickError.value = normalizeError(thrown);
      return null;
    }
  }

  /** Search the sound definitions of the reference set; an empty text lists the first ones. */
  async function searchSounds(query: string): Promise<void> {
    const mine = ++searchSeq;
    sounds.value = { ...sounds.peek(), query, status: 'loading' };
    try {
      const page = await api.searchSounds(query, `sounds-${mine}`);
      if (mine === searchSeq) {
        sounds.value = { query, rows: page.items, total: page.total, status: 'ready' };
      }
    } catch (thrown) {
      if (mine === searchSeq) {
        sounds.value = {
          query,
          rows: [],
          total: 0,
          status: 'failed',
          error: normalizeError(thrown),
        };
      }
    }
  }

  return { facts, sounds, pickError, load, choose, searchSounds };
}

export type AssetStore = ReturnType<typeof createAssetStore>;
