import { Badge, Banner, Button } from 'rimstudio-ui';
import type { ProjectLayoutFixApplyDto } from 'rimstudio-ipc-types';
import { t, tn } from '~/shared/i18n';
import { FIX_KIND_LABEL } from './fixLabels';
import { undoApply, undoFlow } from './fixStore';
import { FixUndoNotice } from './FixUndoNotice';

/** What the apply did: the changes, the files edited, the skipped items with reasons, and the undo. */
export function FixResultView({ result }: { result: ProjectLayoutFixApplyDto }) {
  const undo = undoFlow.value?.applyId === result.applyId ? undoFlow.value : undefined;
  const undone = undo?.result !== undefined;
  const remaining = result.check.issues.length;
  const hasJournal = result.done.length > 0 || result.edited.length > 0;
  return (
    <div class="flex flex-col gap-4">
      {result.cancelled ? (
        <Banner tone="warning" title={t('project.fix.result.cancelled')} />
      ) : null}
      <div class="flex flex-wrap items-center gap-2">
        <Badge tone="success">{tn('project.fix.result.moved', result.done.length)}</Badge>
        {result.edited.length > 0 ? (
          <Badge tone="info">{tn('project.fix.result.edited', result.edited.length)}</Badge>
        ) : null}
        {result.skipped.length > 0 ? (
          <Badge tone="warning">{tn('project.fix.result.skipped', result.skipped.length)}</Badge>
        ) : null}
      </div>
      {result.done.length > 0 ? (
        <section class="flex flex-col gap-1" aria-label={t('project.fix.result.doneList')}>
          <h3 class="m-0 text-body font-semibold">{t('project.fix.result.doneList')}</h3>
          <ul class="m-0 flex list-none flex-col divide-y divide-line-subtle border border-line p-0">
            {result.done.map((item) => (
              <li key={item.id} class="flex flex-col gap-0.5 px-3 py-2">
                <span class="text-small font-semibold">{t(FIX_KIND_LABEL[item.kind])}</span>
                <span class="break-all font-mono text-mono">
                  {item.from && item.from !== item.to ? `${item.from} → ` : ''}
                  {item.to}
                  {item.copied ? (
                    <span class="font-sans text-small text-muted">
                      {' '}
                      ({t('project.fix.result.copied')})
                    </span>
                  ) : null}
                </span>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
      {result.edited.length > 0 ? (
        <section class="flex flex-col gap-1" aria-label={t('project.fix.result.editedList')}>
          <h3 class="m-0 text-body font-semibold">{t('project.fix.result.editedList')}</h3>
          <ul class="m-0 flex list-none flex-col divide-y divide-line-subtle border border-line p-0">
            {result.edited.map((file) => (
              <li key={file.path} class="px-3 py-2 font-mono text-mono break-all">
                {file.path}
                <span class="font-sans text-small text-muted">
                  {' '}
                  (
                  {file.created
                    ? t('project.fix.result.created')
                    : t('project.fix.result.backup', { path: file.backupPath ?? '' })}
                  )
                </span>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
      {result.skipped.length > 0 ? (
        <section class="flex flex-col gap-1" aria-label={t('project.fix.result.skippedList')}>
          <h3 class="m-0 text-body font-semibold">{t('project.fix.result.skippedList')}</h3>
          <ul class="m-0 flex list-none flex-col divide-y divide-line-subtle border border-line p-0">
            {result.skipped.map((item) => (
              <li key={item.id} class="flex flex-col gap-0.5 px-3 py-2">
                <span class="font-mono text-mono">{item.id}</span>
                <span class="text-small">{item.reason}</span>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
      <p class="m-0 text-small text-muted">
        {remaining === 0
          ? t('project.fix.result.remaining.zero')
          : tn('project.fix.result.remaining', remaining)}
      </p>
      {hasJournal ? (
        <div class="flex flex-wrap items-center gap-3">
          <Button
            variant="secondary"
            icon="refresh"
            disabled={undone}
            loading={undo?.running === true}
            onClick={() => void undoApply(result.applyId)}
          >
            {undone ? t('project.fix.undo.done') : t('project.fix.undo')}
          </Button>
          <span class="font-mono text-mono-small text-muted">
            {t('project.fix.result.journal', { id: result.applyId })}
          </span>
        </div>
      ) : null}
      <FixUndoNotice undo={undo} />
    </div>
  );
}
