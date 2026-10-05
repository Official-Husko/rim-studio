import type { DuplicateEntryDto, DuplicateGroupsDto } from 'rimstudio-ipc-types';
import { Badge } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { DUPLICATE_GAME_KEYS, DUPLICATE_REASON_KEYS, SOURCE_KIND_KEYS } from './model';

const HEADERS = [
  ['package', 'setup.dup.col.package'],
  ['kept', 'setup.dup.col.kept'],
  ['skipped', 'setup.dup.col.skipped'],
] as const;

function Entry({ entry }: { entry: DuplicateEntryDto }) {
  return (
    <div class="flex min-w-0 flex-col gap-0.5">
      <div class="flex flex-wrap items-center gap-2">
        <span>{entry.name}</span>
        <Badge>{t(SOURCE_KIND_KEYS[entry.sourceKind])}</Badge>
        <Badge tone={entry.game === 'loaded' ? 'success' : 'warning'}>
          {t(DUPLICATE_GAME_KEYS[entry.game])}
        </Badge>
      </div>
      <span class="font-mono text-mono-small break-all text-muted">{entry.path}</span>
      {entry.why ? <span class="text-small text-muted">{entry.why}</span> : null}
    </div>
  );
}

/** Mods that share a package id: which copy is kept, which are skipped and why. The list is capped. */
export function DuplicateGroups({ duplicates }: { duplicates: DuplicateGroupsDto }) {
  if (duplicates.total === 0) {
    return <p class="m-0 text-small text-muted">{t('setup.dup.none')}</p>;
  }
  return (
    <div class="flex flex-col gap-2">
      <p class="m-0 text-small text-muted" role="status">
        {t('setup.dup.summary', { groups: duplicates.total, skipped: duplicates.skippedTotal })}
        {duplicates.groups.length < duplicates.total
          ? ` ${t('setup.dup.capped', { shown: duplicates.groups.length })}`
          : ''}
      </p>
      <div class="overflow-x-auto">
        <table aria-label={t('setup.dup.title')} class="w-full border-collapse text-body">
          <thead>
            <tr class="border-b border-line bg-raised">
              {HEADERS.map(([key, label]) => (
                <th
                  key={key}
                  scope="col"
                  class="h-control-sm px-3 text-left font-display text-label font-semibold tracking-label text-muted uppercase"
                >
                  {t(label)}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {duplicates.groups.map((group) => (
              <tr key={group.packageId} class="border-b border-line-subtle align-top">
                <td class="min-w-36 px-3 py-2">
                  <div class="font-mono text-mono break-all">{group.packageId}</div>
                  <div class="mt-1 text-small text-muted">
                    {t(DUPLICATE_REASON_KEYS[group.reason])}
                  </div>
                </td>
                <td class="px-3 py-2">
                  <Entry entry={group.kept} />
                </td>
                <td class="px-3 py-2">
                  <ul class="m-0 flex list-none flex-col gap-2 p-0">
                    {group.skipped.map((entry) => (
                      <li key={entry.path}>
                        <Entry entry={entry} />
                      </li>
                    ))}
                  </ul>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
