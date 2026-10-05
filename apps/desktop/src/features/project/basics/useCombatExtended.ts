import { useEffect, useState } from 'preact/hooks';
import type { LibraryModHitDto } from 'rimstudio-ipc-types';
import { searchLibrary } from '../modApi';

/** The package id of Combat Extended. */
export const CE_PACKAGE_ID = 'ceteam.combatextended';

/** Combat Extended as the last library scan found it, or undefined when it is not in the library. */
export function useCombatExtended(): LibraryModHitDto | undefined {
  const [hit, setHit] = useState<LibraryModHitDto | undefined>(undefined);
  useEffect(() => {
    let live = true;
    searchLibrary(CE_PACKAGE_ID, 5).then(
      (found) => {
        if (live) setHit(found.hits.find((h) => h.packageId.toLowerCase() === CE_PACKAGE_ID));
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);
  return hit;
}
