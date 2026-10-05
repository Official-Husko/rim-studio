import { signal, type ReadonlySignal, type Signal } from '@preact/signals';
import { useEffect, useMemo } from 'preact/hooks';
import { normalizeError } from './error';
import { IpcScope } from './lifetime';
import type { ApiError } from './types';

/** The reactive result of a cached query. */
export interface QueryHandle<T> {
  readonly key: string;
  readonly data: ReadonlySignal<T | undefined>;
  readonly error: ReadonlySignal<ApiError | undefined>;
  readonly loading: ReadonlySignal<boolean>;
  /** Run the fetcher again now; resolves when the new result is in. */
  refresh(): Promise<void>;
}

interface Entry<T> {
  data: Signal<T | undefined>;
  error: Signal<ApiError | undefined>;
  loading: Signal<boolean>;
  fetcher?: () => Promise<T>;
  holders: number;
  scoped: boolean;
  fetchedAt: number;
  inflight?: Promise<void>;
}

const cache = new Map<string, Entry<unknown>>();
let clock = (): number => Date.now();

/** Replace the clock (tests). */
export function setQueryClock(fn: () => number): void {
  clock = fn;
}

function entryFor<T>(key: string): Entry<T> {
  let entry = cache.get(key) as Entry<T> | undefined;
  if (!entry) {
    entry = {
      data: signal<T | undefined>(undefined),
      error: signal<ApiError | undefined>(undefined),
      loading: signal(false),
      holders: 0,
      scoped: false,
      fetchedAt: -Infinity,
    };
    cache.set(key, entry as Entry<unknown>);
  }
  return entry;
}

function run<T>(entry: Entry<T>): Promise<void> {
  if (entry.inflight) return entry.inflight;
  const fetcher = entry.fetcher;
  if (!fetcher) return Promise.resolve();
  entry.loading.value = true;
  entry.inflight = fetcher()
    .then((value) => {
      // a result nobody holds any more is dropped, not cached
      if (entry.scoped && entry.holders === 0) return;
      entry.data.value = value;
      entry.error.value = undefined;
      entry.fetchedAt = clock();
    })
    .catch((thrown: unknown) => {
      if (entry.scoped && entry.holders === 0) return;
      entry.error.value = normalizeError(thrown);
    })
    .finally(() => {
      entry.loading.value = false;
      entry.inflight = undefined;
    });
  return entry.inflight;
}

export interface QueryOptions {
  /** Hold the entry for the lifetime of a component scope; dropped results for released keys are not cached. */
  scope?: IpcScope;
  /** Milliseconds a result counts as fresh (default 5000); stale data is shown while it reloads. */
  staleMs?: number;
}

/**
 * A cached query keyed by string. Results are shared by everyone asking for the same key; cached
 * data is returned at once and revalidated when stale (stale while revalidate).
 */
export function query<T>(
  key: string,
  fetcher: () => Promise<T>,
  options: QueryOptions = {},
): QueryHandle<T> {
  const entry = entryFor<T>(key);
  entry.fetcher = fetcher;
  if (options.scope) {
    entry.scoped = true;
    entry.holders += 1;
    options.scope.add(() => {
      entry.holders -= 1;
    });
  }
  const staleMs = options.staleMs ?? 5000;
  if (clock() - entry.fetchedAt >= staleMs) void run(entry);
  return {
    key,
    data: entry.data,
    error: entry.error,
    loading: entry.loading,
    refresh: () => {
      // a refresh waits for a running fetch, then fetches again
      const waiting = entry.inflight ?? Promise.resolve();
      return waiting.then(() => run(entry));
    },
  };
}

/** Mark matching entries stale and reload the ones somebody currently holds. */
export function invalidate(match: string | RegExp | ((key: string) => boolean)): void {
  const test =
    typeof match === 'string'
      ? (k: string) => k === match
      : match instanceof RegExp
        ? (k: string) => match.test(k)
        : match;
  for (const [key, entry] of cache) {
    if (!test(key)) continue;
    entry.fetchedAt = -Infinity;
    if (entry.holders > 0 || !entry.scoped) void run(entry);
  }
}

/** Drop every cached entry (session change, tests). */
export function clearQueries(): void {
  cache.clear();
}

/** Number of cached entries (tests and diagnostics). */
export function queryCacheSize(): number {
  return cache.size;
}

/** Hook form of query(): holds the key while the component is mounted. */
export function useQuery<T>(
  key: string,
  fetcher: () => Promise<T>,
  options: Pick<QueryOptions, 'staleMs'> = {},
): QueryHandle<T> {
  // A new scope and handle per key; the fetcher and options of the first render of a key are used.
  // oxlint-disable-next-line react-hooks/exhaustive-deps
  const scope = useMemo(() => new IpcScope(), [key]);
  // oxlint-disable-next-line react-hooks/exhaustive-deps
  const handle = useMemo(() => query(key, fetcher, { ...options, scope }), [key, scope]);
  useEffect(() => () => scope.dispose(), [scope]);
  return handle;
}
