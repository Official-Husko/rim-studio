import { useEffect } from 'preact/hooks';
import { Banner, Button, EmptyState, Panel, ProgressBar } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import type { ProjectRef } from '~/shared/project';
import { DiagnosticList } from './DiagnosticList';
import { lint, lintEpoch, runLint } from './lintStore';
import { splitDiagnostics } from './model';

export interface LintViewProps {
  project: ProjectRef;
}

/**
 * The lint of the project's Combat Extended conversions: the rule findings (CEP) of every weapon that
 * carries one, and the rules the backend could not check. Reads the project, writes nothing.
 */
export function LintView({ project }: LintViewProps) {
  const state = lint.value;
  const epoch = lintEpoch.value;
  useEffect(() => {
    if (lint.peek().phase === 'idle') void runLint(project);
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [project.path, epoch]);

  if (state.phase === 'idle' || state.phase === 'running') {
    return (
      <ProgressBar
        label={t('patches.lint.running')}
        {...(state.phase === 'running' && state.total > 0
          ? { value: state.done / state.total }
          : {})}
        caption={t('patches.lint.running')}
      />
    );
  }
  if (state.phase === 'error') {
    return (
      <Banner
        tone="error"
        title={state.error.code}
        action={
          <Button size="sm" variant="secondary" onClick={() => void runLint(project)}>
            {t('patches.retry')}
          </Button>
        }
      >
        {state.error.message}
      </Banner>
    );
  }
  if (state.rows.length === 0) {
    return (
      <EmptyState
        compact
        icon="check"
        title={t('patches.lint.none.title')}
        description={t('patches.lint.none.body')}
        action={
          <Button variant="secondary" icon="refresh" onClick={() => void runLint(project)}>
            {t('patches.lint.again')}
          </Button>
        }
      />
    );
  }
  const split = state.rows.map((r) => ({ row: r, parts: splitDiagnostics(r.diagnostics) }));
  const all = split.flatMap((s) => s.parts.other);
  const errors = all.filter((d) => d.severity === 'error').length;
  const warnings = all.filter((d) => d.severity === 'warning').length;
  const notChecked = [
    ...new Map(split.flatMap((s) => s.parts.notChecked).map((d) => [d.message, d])).values(),
  ];
  return (
    <div class="flex flex-col gap-4">
      <div class="flex flex-wrap items-center gap-3">
        <p class="m-0 flex-1 text-body" role="status">
          {t('patches.lint.summary', { weapons: state.rows.length, errors, warnings })}
        </p>
        <Button variant="secondary" icon="refresh" onClick={() => void runLint(project)}>
          {t('patches.lint.again')}
        </Button>
      </div>
      <p class="m-0 text-small text-muted">{t('patches.lint.help')}</p>
      {split.map(({ row, parts }) => (
        <section
          key={row.defName}
          aria-label={row.defName}
          class="flex flex-col gap-2 rounded-sm border border-line p-3"
        >
          <h3 class="m-0 font-mono text-mono font-semibold">{row.defName}</h3>
          {row.error ? (
            <Banner tone="error" title={row.error.code}>
              {row.error.message}
            </Banner>
          ) : parts.other.length === 0 ? (
            <p class="m-0 text-small text-muted">{t('patches.lint.clean')}</p>
          ) : (
            <DiagnosticList
              diagnostics={parts.other}
              label={t('patches.lint.of', { name: row.defName })}
            />
          )}
        </section>
      ))}
      {state.scanDiagnostics.length > 0 ? (
        <Panel title={t('patches.lint.scan')}>
          <DiagnosticList diagnostics={state.scanDiagnostics} label={t('patches.lint.scan')} />
        </Panel>
      ) : null}
      {notChecked.length > 0 ? (
        <Panel
          title={tn('patches.lint.not-checked', notChecked.length)}
          collapsible
          defaultCollapsed
        >
          <p class="m-0 mb-2 text-small text-muted">{t('patches.lint.not-checked-help')}</p>
          <DiagnosticList diagnostics={notChecked} label={t('patches.lint.not-checked.label')} />
        </Panel>
      ) : null}
    </div>
  );
}
