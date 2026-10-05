import { Badge, Banner, Button, Dialog, EmptyState, Spinner } from 'rimstudio-ui';
import type { LayoutFixJournalDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { FixUndoNotice } from './FixUndoNotice';
import { closeHistory, historyFlow, undoApply, undoFlow } from './fixStore';

function JournalRow({ journal }: { journal: LayoutFixJournalDto }) {
  const undo = undoFlow.value;
  const running = undo?.applyId === journal.applyId && undo.running;
  return (
    <li class="flex flex-col gap-1 px-3 py-2">
      <div class="flex flex-wrap items-center gap-2">
        <span class="text-small font-semibold">{new Date(journal.createdMs).toLocaleString()}</span>
        <span class="text-small text-muted">
          {t('project.history.counts', { done: journal.itemsDone, skipped: journal.itemsSkipped })}
        </span>
        {journal.undone ? (
          <Badge tone="neutral">{t('project.history.undone')}</Badge>
        ) : journal.undoPossible ? (
          <Badge tone="success">{t('project.history.possible')}</Badge>
        ) : (
          <Badge tone="warning">{t('project.history.blocked')}</Badge>
        )}
        <span class="flex-1" />
        {journal.undoPossible ? (
          <Button
            size="sm"
            icon="refresh"
            loading={running}
            disabled={undo?.running === true}
            onClick={() => void undoApply(journal.applyId)}
          >
            {t('project.fix.undo.short')}
          </Button>
        ) : null}
      </div>
      {!journal.undoPossible && journal.undoBlocker ? (
        <p class="m-0 text-small text-muted">{journal.undoBlocker}</p>
      ) : null}
      <span class="font-mono text-mono-small text-muted">{journal.applyId}</span>
    </li>
  );
}

/** The list of applied fixes of the project, newest first, with Undo where it is still possible. */
export function FixHistoryDialog() {
  const state = historyFlow.value;
  if (!state.open) return null;
  return (
    <Dialog
      open
      size="lg"
      title={t('project.history.title')}
      closeLabel={t('project.dialog.close')}
      onClose={closeHistory}
      footer={
        <Button variant="primary" onClick={closeHistory}>
          {t('project.fix.close')}
        </Button>
      }
    >
      <div class="flex flex-col gap-3">
        {state.loading && state.journals.length === 0 ? (
          <Spinner label={t('project.fix.loading')} />
        ) : null}
        {state.error ? (
          <Banner tone="error" title={t('project.history.error')}>
            {state.error.message}
          </Banner>
        ) : null}
        <FixUndoNotice undo={undoFlow.value} />
        {!state.loading && !state.error && state.journals.length === 0 ? (
          <EmptyState
            compact
            icon="tasks"
            title={t('project.history.empty.title')}
            description={t('project.history.empty.hint')}
          />
        ) : null}
        {state.journals.length > 0 ? (
          <ul
            class="m-0 flex list-none flex-col divide-y divide-line-subtle border border-line p-0"
            aria-label={t('project.history.list')}
          >
            {state.journals.map((journal) => (
              <JournalRow key={journal.applyId} journal={journal} />
            ))}
          </ul>
        ) : null}
      </div>
    </Dialog>
  );
}
