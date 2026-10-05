import type { CeAmmoTypeDto } from 'rimstudio-ipc-types';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';

export interface AmmoTypeTableProps {
  types: readonly CeAmmoTypeDto[];
}

function num(value: number | undefined): string {
  return value === undefined ? '-' : formatNumber(value, 2);
}

/** The ammunition of one set with the numbers of each projectile. */
export function AmmoTypeTable({ types }: AmmoTypeTableProps) {
  return (
    <div class="overflow-x-auto">
      <table class="w-full border-collapse text-small" aria-label={t('ammo.detail.types')}>
        <thead>
          <tr class="text-left text-faint">
            <th scope="col" class="py-1 pr-2 font-semibold">
              {t('ammo.col.ammo')}
            </th>
            <th scope="col" class="px-2 py-1 text-right font-semibold">
              {t('ammo.col.damage')}
            </th>
            <th scope="col" class="px-2 py-1 text-right font-semibold">
              {t('ammo.col.sharp')}
            </th>
            <th scope="col" class="px-2 py-1 text-right font-semibold">
              {t('ammo.col.blunt')}
            </th>
            <th scope="col" class="px-2 py-1 text-right font-semibold">
              {t('ammo.col.speed')}
            </th>
            <th scope="col" class="px-2 py-1 text-right font-semibold">
              {t('ammo.col.pellets')}
            </th>
          </tr>
        </thead>
        <tbody>
          {types.map((type) => (
            <tr key={type.ammoDef} class="border-t border-line align-top">
              <th scope="row" class="py-1 pr-2 text-left font-normal">
                <span class="block text-fg">{type.ammoLabel}</span>
                <span class="block font-mono text-mono-small text-faint">{type.projectileDef}</span>
                {type.secondaryDamage.length > 0 ? (
                  <span class="block text-mono-small text-muted">
                    {type.secondaryDamage
                      .map((s) => `${s.def} ${formatNumber(s.amount, 1)}`)
                      .join(', ')}
                  </span>
                ) : null}
                {type.explosionRadius !== undefined ? (
                  <span class="block text-mono-small text-muted">
                    {t('ammo.detail.radius', { value: formatNumber(type.explosionRadius, 1) })}
                  </span>
                ) : null}
              </th>
              <td class="px-2 py-1 text-right font-mono tabular-nums">{num(type.damage)}</td>
              <td class="px-2 py-1 text-right font-mono tabular-nums">
                {num(type.armorPenetrationSharp)}
              </td>
              <td class="px-2 py-1 text-right font-mono tabular-nums">
                {num(type.armorPenetrationBlunt)}
              </td>
              <td class="px-2 py-1 text-right font-mono tabular-nums">{num(type.speed)}</td>
              <td class="px-2 py-1 text-right font-mono tabular-nums">{num(type.pellets)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
