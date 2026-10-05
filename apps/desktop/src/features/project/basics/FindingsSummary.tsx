import { t, tn } from '~/shared/i18n';
import { FieldFindings } from './FieldFindings';
import { isShownInline } from './aboutModel';
import { findings } from './aboutStore';

/** The totals of the findings, and the findings that belong to no field of the form (the file as a whole). */
export function FindingsSummary() {
  const all = findings.value;
  const errors = all.filter((d) => d.severity === 'error').length;
  const warnings = all.filter((d) => d.severity === 'warning').length;
  const notes = all.length - errors - warnings;
  const loose = all.filter((d) => !isShownInline(d));
  return (
    <section
      aria-label={t('project.basics.findings')}
      class="flex flex-col gap-2 border border-line bg-surface px-3 py-2"
    >
      <p class="m-0 text-small text-muted" aria-live="polite">
        {all.length === 0
          ? t('project.basics.findings.none')
          : t('project.basics.findings.counts', {
              errors: tn('project.basics.findings.error', errors),
              warnings: tn('project.basics.findings.warning', warnings),
              notes: tn('project.basics.findings.note', notes),
            })}
      </p>
      <FieldFindings items={loose} />
    </section>
  );
}
