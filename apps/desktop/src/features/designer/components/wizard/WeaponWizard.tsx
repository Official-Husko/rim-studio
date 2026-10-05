import { useEffect, useMemo, useRef } from 'preact/hooks';
import type { DraftDto } from 'rimstudio-ipc-types';
import { Banner, Button, Dialog } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { currentProject } from '../../project-source';
import type { Scheduler } from '../../scheduler';
import type { Designer } from '../../stores';
import { CategoryStep } from './CategoryStep';
import { DescribeStep } from './DescribeStep';
import { LiveSummary } from './LiveSummary';
import { NameStep } from './NameStep';
import { ResultStep } from './ResultStep';
import { StepHeader, stepLabel } from './StepHeader';
import { WIZARD_STEPS, type WizardStep } from './wizard-model';
import { createWizardStore } from './wizard-store';

export interface WeaponWizardProps {
  /** The designer that receives the new draft, or the open draft that is retuned. */
  stores: Designer;
  /** The open draft to tune again; without it the wizard makes a new draft. */
  retune?: DraftDto | undefined;
  /** Opens the blank draft dialog instead (new drafts only). */
  onBlank?: (() => void) | undefined;
  onClose: () => void;
  /** The id of the current project; defaults to the project of the top bar. */
  projectId?: () => string | undefined;
  /** Replaced by a manual one in tests. */
  scheduler?: Scheduler | undefined;
}

const RETUNE_STEPS: readonly WizardStep[] = ['describe', 'result'];

/**
 * The new weapon wizard: pick the type, describe the gun, read the proposed numbers, name it. The
 * draft is created with the proposal applied and opened in the editor; nothing is written to the
 * project. With `retune` it proposes again for an open draft and keeps the numbers the user typed.
 */
export function WeaponWizard({
  stores,
  retune,
  onBlank,
  onClose,
  scheduler,
  projectId = () => currentProject()?.projectId,
}: WeaponWizardProps) {
  // the reader is called when needed, so a new closure on every render must not rebuild the store
  const reader = useRef(projectId);
  reader.current = projectId;
  const store = useMemo(
    () =>
      createWizardStore({
        projectId: () => reader.current(),
        onCreated: (entry) => {
          stores.drafts.upsert(entry);
          void stores.select(entry);
        },
        onApplied: (draft) => stores.editor.update(() => draft),
        ...(scheduler ? { scheduler } : {}),
      }),
    [stores, scheduler],
  );
  // the draft is read once: edits that arrive while the dialog is open must not restart it
  const initial = useRef(retune);
  useEffect(() => {
    void store.loadCatalog().then(() => {
      if (initial.current) store.retune(initial.current);
    });
    return () => store.reset();
  }, [store]);

  const steps = retune ? RETUNE_STEPS : WIZARD_STEPS;
  const step = store.step.value;
  const at = steps.indexOf(step);
  const last = at === steps.length - 1;
  const choice = store.choice.value;
  const needsAmmo = choice.ceCalibre && choice.descriptors.ammoSet === undefined;
  const canGo = (target: WizardStep): boolean => {
    if (target === 'category') return true;
    if (choice.archetypeId === undefined) return false;
    if (target === 'name') return store.proposal.value !== undefined;
    return true;
  };
  const canNext =
    (step === 'category' && choice.archetypeId !== undefined) ||
    (step === 'describe' && !needsAmmo) ||
    (step === 'result' && store.proposal.value !== undefined && !store.proposeError.value);
  const finish = async (): Promise<void> => {
    const done = retune ? await store.applyToTarget() : await store.create();
    if (done) onClose();
  };
  const submitDisabled =
    last && (retune ? store.proposal.value === undefined : store.defName.value.trim() === '');
  const title = retune ? t('designer.wizard.retuneTitle') : t('designer.wizard.title');
  return (
    <Dialog
      open
      size="full"
      title={title}
      onClose={onClose}
      closeLabel={t('designer.dialog.close')}
      footer={
        <div class="flex w-full flex-wrap items-center justify-between gap-2">
          <div>
            {!retune && step === 'category' && onBlank ? (
              <Button variant="ghost" onClick={onBlank}>
                {t('designer.wizard.blank')}
              </Button>
            ) : null}
          </div>
          <div class="flex gap-2">
            <Button onClick={onClose}>{t('designer.dialog.cancel')}</Button>
            {at > 0 ? (
              <Button onClick={() => store.move(-1)}>{t('designer.wizard.back')}</Button>
            ) : null}
            {last ? (
              <Button
                variant="primary"
                loading={store.busy.value}
                disabled={submitDisabled}
                onClick={() => void finish()}
              >
                {retune ? t('designer.wizard.apply') : t('designer.dialog.create')}
              </Button>
            ) : (
              <Button variant="primary" disabled={!canNext} onClick={() => store.move(1)}>
                {t('designer.wizard.next')}
              </Button>
            )}
          </div>
        </div>
      }
    >
      <div class="flex min-h-0 flex-1 flex-col gap-4">
        <StepHeader steps={steps} current={step} reachable={canGo} onGo={store.go} />
        {retune && store.error.value ? (
          <Banner tone="error" title={store.error.value.code}>
            {store.error.value.message}
          </Banner>
        ) : null}
        <div class="flex min-h-0 flex-1 flex-col gap-4 lg:flex-row">
          <div class="min-w-0 flex-1" role="region" aria-label={stepLabel(step)}>
            {step === 'category' ? (
              <CategoryStep
                catalog={store.catalog.value}
                error={store.catalogError.value}
                chosen={choice.archetypeId}
                onChoose={store.chooseArchetype}
              />
            ) : null}
            {step === 'describe' ? <DescribeStep store={store} /> : null}
            {step === 'result' ? <ResultStep store={store} /> : null}
            {step === 'name' ? <NameStep store={store} onSubmit={() => void finish()} /> : null}
          </div>
          {step !== 'category' ? (
            <aside
              class="shrink-0 border-t border-line pt-3 lg:sticky lg:top-0 lg:w-72 lg:self-start lg:border-t-0 lg:border-l lg:pt-0 lg:pl-4"
              aria-label={t('designer.wizard.summary.title')}
            >
              <LiveSummary store={store} />
            </aside>
          ) : null}
        </div>
      </div>
    </Dialog>
  );
}
