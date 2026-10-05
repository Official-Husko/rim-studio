import type { CeAmmoEntryDto } from 'rimstudio-ipc-types';
import { Badge } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';

export interface AmmoRowProps {
  entry: CeAmmoEntryDto;
}

/** Height of one row of the list in pixels (the list is windowed, so every row has the same height). */
export const AMMO_ROW_HEIGHT = 56;

/** One ammo set in the list: its label, the caliber, the kinds of ammunition and how many weapons use it. */
export function AmmoRow({ entry }: AmmoRowProps) {
  const kinds = entry.types.map((type) => type.ammoClassLabel).join(', ');
  return (
    <div class="flex h-full min-w-0 flex-col justify-center gap-0.5 px-3">
      <div class="flex min-w-0 items-baseline gap-2">
        <span class="truncate text-body font-semibold text-fg">{entry.label}</span>
        {entry.suggested ? <Badge tone="info">{t('ammo.row.suggested')}</Badge> : null}
        {entry.generic ? <Badge>{t('ammo.row.generic')}</Badge> : null}
        <span class="ml-auto shrink-0 font-mono text-mono-small text-faint">{entry.defName}</span>
      </div>
      <div class="flex min-w-0 items-baseline gap-2 text-small text-muted">
        <span class="shrink-0">{tn('ammo.row.types', entry.types.length)}</span>
        <span class="truncate">{kinds}</span>
        <span class="ml-auto shrink-0">
          {entry.weaponCount > 0
            ? tn('ammo.row.weapons', entry.weaponCount)
            : t('ammo.row.no-weapons')}
        </span>
      </div>
    </div>
  );
}
