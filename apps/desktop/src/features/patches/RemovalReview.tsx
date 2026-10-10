import type { FileActionDto, PlannedFileDto, WritePlanDto } from 'rimstudio-ipc-types';
import { Badge, Banner, DiffView } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { DiagnosticList } from './DiagnosticList';
import type { RemovalTarget } from './removeStore';

/** What a removal does to a file, in words. A removal plan holds only these two actions. */
export function removalActionText(action: FileActionDto): string {
  return action === 'delete'
    ? t('patches.remove.action.delete')
    : t('patches.remove.action.update-region');
}

function RemovalFile({ file }: { file: PlannedFileDto }) {
  return (
    <section class="flex flex-col gap-2 rounded-sm border border-line p-3" aria-label={file.path}>
      <div class="flex flex-wrap items-center gap-x-3 gap-y-1">
        <code class="min-w-0 font-mono text-mono break-all">{file.path}</code>
        <Badge tone={file.action === 'delete' ? 'danger' : 'info'}>
          {removalActionText(file.action)}
        </Badge>
      </div>
      {file.diff ? (
        <DiffView
          diff={file.diff}
          label={t('patches.file.diff-label', { path: file.path })}
          class="max-h-72"
        />
      ) : null}
    </section>
  );
}

export interface RemovalReviewProps {
  target: RemovalTarget;
  plan: WritePlanDto;
}

/**
 * The review of a removal: what is taken out, file by file, as a diff, and what stays as it is. A plan with
 * an error lists its diagnostics and nothing else.
 */
export function RemovalReview({ target, plan }: RemovalReviewProps) {
  const heading = tn('patches.remove.files', plan.files.length);
  return (
    <div class="flex flex-col gap-4">
      <p class="m-0 text-body">{t('patches.remove.intro', { name: target.name })}</p>
      {target.keepsConversion ? <p class="m-0 text-body">{t('patches.existing.keeps')}</p> : null}
      {target.mode === 'replace' ? (
        <p class="m-0 text-body">{t('patches.remove.intro-replace')}</p>
      ) : null}
      {plan.hasErrors ? (
        <Banner tone="error" title={t('patches.remove.blocked')}>
          <DiagnosticList diagnostics={plan.diagnostics} label={t('patches.remove.blocked')} />
        </Banner>
      ) : plan.diagnostics.length > 0 ? (
        <DiagnosticList diagnostics={plan.diagnostics} label={t('patches.plan.notes')} />
      ) : null}
      {plan.files.length > 0 ? (
        <>
          <section aria-label={heading} class="flex flex-col gap-2">
            <h3 class="m-0 font-display text-label font-semibold tracking-label text-muted uppercase">
              {heading}
            </h3>
            {plan.files.map((file) => (
              <RemovalFile key={file.path} file={file} />
            ))}
          </section>
          <ul class="m-0 flex list-none flex-col gap-1 p-0 text-small text-muted">
            <li>{t('patches.remove.backup')}</li>
            <li>{t('patches.remove.defs-kept')}</li>
          </ul>
        </>
      ) : !plan.hasErrors ? (
        <Banner tone="info">{t('patches.plan.nothing')}</Banner>
      ) : null}
    </div>
  );
}
