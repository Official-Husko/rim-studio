import { useEffect } from 'preact/hooks';
import { Banner, Button, Dialog } from 'rimstudio-ui';
import { t, type MessageKey } from '~/shared/i18n';
import { IdentityStep } from './create/IdentityStep';
import { ReviewStep } from './create/ReviewStep';
import { StructureStep } from './create/StructureStep';
import {
  STEPS,
  createChecking,
  createForm,
  createPreview,
  createStep,
  createTarget,
  goStep,
  identityReady,
  openCreate,
  type CreateStep,
} from './create/createStore';
import { requestOf } from './scaffold';
import { createError, createMod, creating } from './store';

export interface NewModDialogProps {
  open: boolean;
  onClose: () => void;
  /** The parent folder to start with, when the user has one in mind. */
  startParent?: string;
}

const STEP_LABEL: Record<CreateStep, MessageKey> = {
  identity: 'project.create.step.identity',
  structure: 'project.create.step.structure',
  review: 'project.create.step.review',
};

/** The new mod window: identity, the recommended structure with its options, then a review of what is written. */
export function NewModDialog({ open, onClose, startParent }: NewModDialogProps) {
  useEffect(() => {
    if (open) openCreate(startParent ?? '');
  }, [open, startParent]);

  const step = createStep.value;
  const index = STEPS.indexOf(step);
  const preview = createPreview.value;
  const canCreate = identityReady.value && (preview?.conflicts.length ?? 0) === 0;

  const submit = async (): Promise<void> => {
    if (!canCreate) return;
    if (await createMod(requestOf(createForm.peek(), createTarget.peek()))) onClose();
  };
  const next = STEPS[index + 1];
  const back = STEPS[index - 1];

  return (
    <Dialog
      open={open}
      title={t('project.new.title')}
      size="lg"
      onClose={onClose}
      closeLabel={t('project.dialog.close')}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('project.dialog.cancel')}
          </Button>
          {back ? (
            <Button variant="secondary" onClick={() => goStep(back)}>
              {t('project.create.back')}
            </Button>
          ) : null}
          {next ? (
            <Button disabled={!identityReady.value} onClick={() => goStep(next)}>
              {t('project.create.next')}
            </Button>
          ) : (
            <Button disabled={!canCreate} loading={creating.value} onClick={() => void submit()}>
              {t('project.new.create')}
            </Button>
          )}
        </>
      }
    >
      <div class="flex flex-col gap-4">
        <ol class="m-0 flex list-none flex-wrap gap-4 p-0" aria-label={t('project.create.steps')}>
          {STEPS.map((id, i) => (
            <li
              key={id}
              aria-current={id === step ? 'step' : undefined}
              class={id === step ? 'font-semibold text-fg' : 'text-muted'}
            >
              {`${i + 1}. ${t(STEP_LABEL[id])}`}
            </li>
          ))}
        </ol>
        {step === 'identity' ? (
          <IdentityStep />
        ) : step === 'structure' ? (
          <StructureStep />
        ) : (
          <ReviewStep />
        )}
        {createChecking.value ? (
          <p class="m-0 text-small text-faint" aria-live="polite">
            {t('project.create.checking')}
          </p>
        ) : null}
        {createError.value ? (
          <Banner tone="error" title={t('project.new.error')}>
            {createError.value.message}
          </Banner>
        ) : null}
      </div>
    </Dialog>
  );
}
