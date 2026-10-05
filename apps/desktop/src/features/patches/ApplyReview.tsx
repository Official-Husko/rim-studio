import type { ConvertCandidateDto, FileActionDto, FileKindDto } from 'rimstudio-ipc-types';
import { Banner, Switch } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { isReady, type ReviewItem } from './applyStore';
import { splitDiagnostics } from './model';

export interface ApplyReviewProps {
  items: readonly ReviewItem[];
  backup: boolean;
  dryApply: boolean;
  onBackup: (on: boolean) => void;
  onDryApply: (on: boolean) => void;
}

interface FileRow {
  path: string;
  kind: FileKindDto;
  action: FileActionDto;
  weapons: string[];
}

/** The files of every ready plan, one row per path. */
export function fileRows(items: readonly ReviewItem[]): FileRow[] {
  const rows = new Map<string, FileRow>();
  for (const item of items) {
    if (!isReady(item.entry) || !item.entry.plan) continue;
    for (const file of item.entry.plan.files) {
      // the weapons of a mod share their files; the first plan decides what is shown for a path
      const row = rows.get(file.path) ?? {
        path: file.path,
        kind: file.kind,
        action: file.action,
        weapons: [],
      };
      row.weapons.push(item.candidate.defName);
      rows.set(file.path, row);
    }
  }
  return [...rows.values()];
}

function why(candidate: ConvertCandidateDto, item: ReviewItem): string {
  if (item.entry.phase === 'error') return item.entry.error?.message ?? '';
  const open = item.entry.plan ? splitDiagnostics(item.entry.plan.diagnostics).open.length : 0;
  return open > 0
    ? tn('patches.plan.open', open)
    : t('patches.apply.nothing-to-write', { name: candidate.defName });
}

/** The confirmation: what will be written, the LoadFolders.xml edit, the backup choice. */
export function ApplyReview({ items, backup, dryApply, onBackup, onDryApply }: ApplyReviewProps) {
  const ready = items.filter((i) => isReady(i.entry));
  const blocked = items.filter((i) => !isReady(i.entry));
  const rows = fileRows(items);
  const loadFolders = rows.find((r) => r.kind === 'load-folders');
  const replaced = rows.filter((r) => r.action === 'update-region' || r.weapons.length > 1);
  return (
    <div class="flex flex-col gap-4">
      <p class="m-0 text-body">{tn('patches.apply.weapons', ready.length)}</p>
      <p class="m-0 font-mono text-mono-small text-muted">
        {ready.map((i) => i.candidate.defName).join(', ')}
      </p>
      {blocked.length > 0 ? (
        <Banner tone="warning" title={tn('patches.apply.blocked', blocked.length)}>
          <ul class="m-0 list-none p-0">
            {blocked.map((i) => (
              <li key={i.candidate.defName}>
                <span class="font-mono">{i.candidate.defName}</span>: {why(i.candidate, i)}
              </li>
            ))}
          </ul>
        </Banner>
      ) : null}
      <section aria-label={t('patches.apply.files')} class="flex flex-col gap-1">
        <h3 class="m-0 font-display text-label font-semibold tracking-label text-muted uppercase">
          {t('patches.apply.files')}
        </h3>
        <ul class="m-0 flex list-none flex-col gap-1 p-0">
          {rows.map((r) => (
            <li key={r.path} class="flex flex-wrap items-baseline gap-x-2 text-small">
              <code class="font-mono text-mono">{r.path}</code>
              {r.weapons.length > 1 ? (
                <span class="text-muted">{t('patches.apply.shared', { n: r.weapons.length })}</span>
              ) : null}
              <span class="text-muted">
                {r.action === 'create'
                  ? t('patches.action.create')
                  : r.action === 'update-region'
                    ? t('patches.action.update-region')
                    : t('patches.action.unchanged')}
              </span>
            </li>
          ))}
        </ul>
      </section>
      {loadFolders ? (
        <Banner tone="info" title={t('patches.apply.loadfolders')}>
          {loadFolders.action === 'create'
            ? t('patches.apply.loadfolders-create')
            : t('patches.apply.loadfolders-edit')}
        </Banner>
      ) : null}
      <section class="flex flex-col gap-2" aria-label={t('patches.apply.options')}>
        <Switch checked={backup} onCheckedChange={onBackup}>
          {t('patches.apply.backup')}
        </Switch>
        <p class="m-0 text-small text-muted">
          {replaced.length > 0 ? t('patches.apply.backup-where') : t('patches.apply.backup-none')}
        </p>
        <Switch checked={dryApply} onCheckedChange={onDryApply}>
          {t('patches.apply.dry')}
        </Switch>
      </section>
    </div>
  );
}
