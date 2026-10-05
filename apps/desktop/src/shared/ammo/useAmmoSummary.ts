import { useEffect, useState } from 'preact/hooks';
import type { CeAmmoEntryDto } from 'rimstudio-ipc-types';
import { ammoCatalog } from './api';

const known = new Map<string, CeAmmoEntryDto>();

/** Forget the sets looked up so far (tests, and a change of the install). */
export function clearAmmoSummaries(): void {
  known.clear();
}

/**
 * The catalogue entry of one ammo set, looked up by its def name, so the field can show the caliber and the
 * ammo types of the chosen set. `undefined` while it loads or when the set is not in the install.
 */
export function useAmmoSummary(
  defName: string | undefined,
  fetchPage: typeof ammoCatalog = ammoCatalog,
): CeAmmoEntryDto | undefined {
  const [entry, setEntry] = useState<CeAmmoEntryDto | undefined>(
    defName ? known.get(defName) : undefined,
  );
  useEffect(() => {
    if (!defName) {
      setEntry(undefined);
      return undefined;
    }
    const held = known.get(defName);
    if (held) {
      setEntry(held);
      return undefined;
    }
    setEntry(undefined);
    let live = true;
    fetchPage({ query: defName, pageSize: 10 })
      .then((page) => {
        const found = page.entries.find((e) => e.defName === defName);
        if (found) known.set(defName, found);
        if (live) setEntry(found);
      })
      .catch(() => {
        if (live) setEntry(undefined);
      });
    return () => {
      live = false;
    };
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [defName]);
  return entry;
}
