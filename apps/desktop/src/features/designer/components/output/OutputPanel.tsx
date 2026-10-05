import { useEffect, useRef } from 'preact/hooks';
import { Banner, EmptyState, Panel, Spinner } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { focusField } from '../../model/draft';
import { parseOutputLinks, runOutputLinks } from '../../output-dev-links';
import { isDerived, isLint, problemsOf } from '../../output-model';
import { designer, type Designer } from '../../stores';
import type { OutputStore } from '../../output-store';
import { ApplyBar } from './ApplyBar';
import { ApplyDialog } from './ApplyDialog';
import { CeLint } from './CeLint';
import { CeSection } from './CeSection';
import { DerivedValues } from './DerivedValues';
import { FileList } from './FileList';
import { FilePreview } from './FilePreview';
import { PlanProblems } from './PlanProblems';

export interface OutputPanelProps {
  /** The stored id of the open draft. */
  draftId: string | undefined;
  /** The folder of the current project. */
  projectPath: string | undefined;
  /** The output store of the page; the running app uses the shared one. */
  store?: OutputStore;
  /** The editor of the page, for the draft the output follows. */
  editor?: Designer['editor'];
}

/**
 * The output of the open draft: the files the plan would write, a preview of each, the problems
 * that block writing, the optional Combat Extended patch with its questions, and the Apply button
 * behind a confirmation. The plan follows every edit; nothing is written without the dialog.
 */
export function OutputPanel({
  draftId,
  projectPath,
  store = designer.output,
  editor = designer.editor,
}: OutputPanelProps) {
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => store.start(), [store]);
  useEffect(() => {
    if (!import.meta.env.DEV) return undefined;
    return runOutputLinks(store, editor, parseOutputLinks(location.hash));
  }, [store, editor]);

  const draft = editor.draft.value;
  if (!draftId || !projectPath || !draft) {
    return (
      <Panel title={t('designer.panel.output')}>
        <p class="text-small text-muted">{t('designer.output.noDraft')}</p>
      </Panel>
    );
  }

  const plan = store.plan.value;
  const selected = plan?.files.find((f) => f.path === store.selectedPath.value) ?? plan?.files[0];
  const busy = store.planning.value || store.stale.value;
  const error = store.planError.value;
  const on = store.ceOn.value;

  const goTo = (pointer: string): void => {
    const holder = root.current;
    if (pointer.startsWith('/ce/') && holder) {
      const row = Array.from(holder.querySelectorAll<HTMLElement>('[data-ce-field]')).find(
        (el) => el.dataset.ceField === pointer,
      );
      const control = row?.querySelector<HTMLElement>('input, select, button');
      if (control) {
        control.focus();
        control.scrollIntoView?.({ block: 'center' });
        return;
      }
    }
    focusField(document, pointer);
  };

  return (
    <div ref={root} class="flex flex-col gap-3">
      <Panel
        title={t('designer.panel.output')}
        actions={
          busy ? (
            <span class="inline-flex items-center gap-1 text-small text-muted">
              <Spinner label={t('designer.output.planning')} />
              {t('designer.output.planning')}
            </span>
          ) : plan ? (
            <span class="font-mono text-mono-small text-muted">
              {tn('designer.output.fileCount', plan.files.length)}
            </span>
          ) : null
        }
      >
        <div class="flex flex-col gap-3">
          {error ? (
            <Banner tone="error" title={error.code}>
              {error.message}
            </Banner>
          ) : null}
          {plan ? (
            <>
              <FileList files={plan.files} selected={selected?.path} onSelect={store.select} />
              {on && plan.hasErrors && !plan.files.some((f) => f.kind === 'ce-patch') ? (
                <p class="text-small text-muted">{t('designer.output.patchWaits')}</p>
              ) : null}
              {selected ? <FilePreview key={selected.path} file={selected} /> : null}
            </>
          ) : error ? null : (
            <EmptyState compact title={t('designer.output.planning')} />
          )}
          <ApplyBar store={store} />
        </div>
      </Panel>
      <CeSection store={store} spec={draft.spec} onGoTo={goTo} />
      <PlanProblems problems={problemsOf(plan)} onGoTo={goTo} />
      {on && plan ? <CeLint lint={plan.diagnostics.filter(isLint)} /> : null}
      <DerivedValues derived={(plan?.diagnostics ?? []).filter(isDerived)} />
      <ApplyDialog store={store} projectPath={projectPath} />
    </div>
  );
}
