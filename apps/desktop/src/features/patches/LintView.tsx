import { useEffect } from 'preact/hooks';
import { Banner, Button, EmptyState, Panel, ProgressBar } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import type { ProjectRef } from '~/shared/project';
import { DiagnosticList } from './DiagnosticList';
import { LintFileSection } from './LintFileSection';
import { LintFindingRow } from './LintFindingRow';
import { lint, lintEpoch, runLint } from './lintStore';

export interface LintViewProps {
  project: ProjectRef;
}

/**
 * The lint of the project's patch files, hand written and generated: the findings grouped by file with
 * the rule explanation and the operation, and the rules the backend could not check. Reads the project,
 * writes nothing.
 */
export function LintView({ project }: LintViewProps) {
  const state = lint.value;
  const epoch = lintEpoch.value;
  useEffect(() => {
    if (lint.peek().phase === 'idle') void runLint(project);
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [project.path, epoch]);

  if (state.phase === 'idle' || state.phase === 'running') {
    return <ProgressBar label={t('patches.lint.running')} caption={t('patches.lint.running')} />;
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
  const { result } = state;
  const again = (
    <Button variant="secondary" icon="refresh" onClick={() => void runLint(project)}>
      {t('patches.lint.again')}
    </Button>
  );
  if (result.files.length === 0 && result.project.length === 0) {
    return (
      <EmptyState
        compact
        icon="check"
        title={t('patches.lint.none.title')}
        description={t('patches.lint.none.body')}
        action={again}
      />
    );
  }
  return (
    <div class="flex flex-col gap-4">
      <div class="flex flex-wrap items-center gap-3">
        <p class="m-0 flex-1 text-body" role="status">
          {t('patches.lint.summary', {
            files: result.counts.checked,
            errors: result.counts.errors,
            warnings: result.counts.warnings,
            notes: result.counts.notes,
          })}
        </p>
        {again}
      </div>
      <p class="m-0 text-small text-muted">
        {t('patches.lint.help', { version: result.gameVersion })}
      </p>
      {result.ceData ? null : <Banner tone="info">{t('patches.lint.no-ce-data')}</Banner>}
      {result.project.length > 0 ? (
        <Panel title={t('patches.lint.project')}>
          <ul aria-label={t('patches.lint.project')} class="m-0 flex list-none flex-col gap-3 p-0">
            {result.project.map((finding, index) => (
              <LintFindingRow key={`${finding.code}-${index}`} finding={finding} />
            ))}
          </ul>
        </Panel>
      ) : null}
      {result.files.map((file) => (
        <LintFileSection key={file.path} file={file} />
      ))}
      {result.diagnostics.length > 0 ? (
        <Panel title={t('patches.lint.scan')}>
          <DiagnosticList diagnostics={result.diagnostics} label={t('patches.lint.scan')} />
        </Panel>
      ) : null}
      {result.notChecked.length > 0 ? (
        <Panel
          title={tn('patches.lint.not-checked', result.notChecked.length)}
          collapsible
          defaultCollapsed
        >
          <p class="m-0 mb-2 text-small text-muted">{t('patches.lint.not-checked-help')}</p>
          <ul
            aria-label={t('patches.lint.not-checked.label')}
            class="m-0 flex list-none flex-col gap-2 p-0 text-small"
          >
            {result.notChecked.map((rule) => (
              <li key={rule.ruleId} class="flex flex-wrap items-baseline gap-x-2">
                <code class="font-mono text-mono-small font-semibold">{rule.ruleId}</code>
                <span class="text-body">{rule.reason}</span>
              </li>
            ))}
          </ul>
        </Panel>
      ) : null}
    </div>
  );
}
