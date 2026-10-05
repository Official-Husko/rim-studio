import { useEffect, useRef, useState } from 'preact/hooks';
import { SegmentedControl } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { projectRevision, useCurrentProject } from '~/shared/project';
import { resetAnswers } from './answerStore';
import { ApplyDialog } from './ApplyDialog';
import { applyScanLinks, devLink, openFromLink } from './devLinks';
import { ConvertView } from './ConvertView';
import { LintView } from './LintView';
import { resetLint } from './lintStore';
import { NoProject } from './NoProject';
import { resetPlans } from './planStore';
import { resetScan, runScan, scan } from './scanStore';
import { resetChoices } from './suggestStore';
import { PROJECT_MISSING } from './session';

/**
 * Patches: the Combat Extended patch generator for the weapons of an existing mod. Scan the mod, answer
 * what the generator cannot derive, review the plan per weapon, apply, and lint the result.
 */
export default function PatchesPage() {
  const project = useCurrentProject();
  const revision = projectRevision.value;
  const [view, setView] = useState<'convert' | 'lint'>(() =>
    devLink('view') === 'lint' ? 'lint' : 'convert',
  );
  const lastRevision = useRef(revision);

  useEffect(() => {
    void openFromLink();
  }, []);

  useEffect(() => {
    resetScan();
    resetAnswers();
    resetPlans();
    resetChoices();
    resetLint();
    if (project) void runScan(project);
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [project?.path]);

  useEffect(() => {
    if (revision === lastRevision.current) return;
    lastRevision.current = revision;
    resetPlans();
    resetLint();
    if (project) void runScan(project);
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [revision]);

  const state = scan.value;
  const scanned = state.phase === 'ready' ? state.data : undefined;
  useEffect(() => {
    if (scanned) applyScanLinks(scanned.candidates);
  }, [scanned]);
  const missing =
    state.phase === 'error' && state.error.code === PROJECT_MISSING
      ? { path: state.path, reason: state.error.message }
      : undefined;

  return (
    <div class="flex h-full min-h-0 flex-col gap-4 p-6">
      <div class="flex flex-wrap items-center gap-3">
        <h1 class="m-0 flex-1 font-display text-display font-semibold tracking-display">
          {t('patches.title')}
        </h1>
        {project && !missing ? (
          <SegmentedControl
            label={t('patches.view.label')}
            value={view}
            onValueChange={(v) => setView(v === 'lint' ? 'lint' : 'convert')}
            options={[
              { value: 'convert', label: t('patches.view.convert') },
              { value: 'lint', label: t('patches.view.lint') },
            ]}
          />
        ) : null}
      </div>
      {project ? (
        <p class="m-0 text-small text-muted">
          <span class="text-fg">{project.name}</span>{' '}
          <code class="font-mono text-mono-small">{project.path}</code>
        </p>
      ) : null}
      {!project || missing ? <NoProject missing={missing} /> : null}
      {project && !missing && view === 'convert' ? <ConvertView project={project} /> : null}
      {project && !missing && view === 'lint' ? (
        <div class="min-h-0 flex-1 overflow-auto">
          <LintView project={project} />
        </div>
      ) : null}
      {project ? <ApplyDialog project={project} /> : null}
    </div>
  );
}
