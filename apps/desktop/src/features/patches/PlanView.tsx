import type { ConvertCandidateDto } from 'rimstudio-ipc-types';
import { Banner, Button, Panel, Spinner } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { DerivedTable } from './DerivedTable';
import { DiagnosticList } from './DiagnosticList';
import { splitDiagnostics } from './model';
import { PlanFile } from './PlanFile';
import type { PlanEntry } from './planStore';

export interface PlanViewProps {
  candidate: ConvertCandidateDto;
  entry: PlanEntry | undefined;
  onRetry: () => void;
  onOpenQuestions: () => void;
}

/** The generated plan of one weapon: the files, the derived numbers and the remaining diagnostics. */
export function PlanView({ candidate, entry, onRetry, onOpenQuestions }: PlanViewProps) {
  if (!entry || entry.phase === 'loading') {
    return <Spinner label={t('patches.plan.loading')} />;
  }
  if (entry.phase === 'error' || !entry.plan) {
    return (
      <Banner
        tone="error"
        title={entry.error?.code ?? 'error'}
        action={
          <Button size="sm" variant="secondary" onClick={onRetry}>
            {t('patches.retry')}
          </Button>
        }
      >
        {entry.error?.message}
      </Banner>
    );
  }
  const plan = entry.plan;
  const split = splitDiagnostics(plan.diagnostics);
  return (
    <div class="flex flex-col gap-4">
      {split.open.length > 0 ? (
        <Banner
          tone="warning"
          title={tn('patches.plan.open', split.open.length)}
          action={
            <Button size="sm" variant="secondary" onClick={onOpenQuestions}>
              {t('patches.plan.go-questions')}
            </Button>
          }
        >
          {t('patches.plan.open-body')}
        </Banner>
      ) : plan.files.length === 0 ? (
        <Banner tone="info">
          {candidate.status === 'already-ce'
            ? t('patches.plan.nothing-converted')
            : t('patches.plan.nothing')}
        </Banner>
      ) : (
        <Banner tone="success">{tn('patches.plan.ready', plan.files.length)}</Banner>
      )}
      {plan.files.length > 0 ? (
        <Panel title={t('patches.plan.files')}>
          <div class="flex flex-col gap-3">
            {plan.files.map((file) => (
              <PlanFile key={file.path} file={file} />
            ))}
          </div>
        </Panel>
      ) : null}
      {split.derived.length > 0 ? (
        <Panel title={t('patches.plan.derived')}>
          <p class="m-0 mb-2 text-small text-muted">{t('patches.plan.derived-help')}</p>
          <DerivedTable diagnostics={split.derived} />
        </Panel>
      ) : null}
      {split.other.length > 0 ? (
        <Panel title={t('patches.plan.notes')}>
          <DiagnosticList diagnostics={split.other} label={t('patches.plan.notes')} />
        </Panel>
      ) : null}
      {split.notChecked.length > 0 ? (
        <Panel title={t('patches.plan.not-checked')} collapsible defaultCollapsed>
          <DiagnosticList diagnostics={split.notChecked} label={t('patches.plan.not-checked')} />
        </Panel>
      ) : null}
    </div>
  );
}
