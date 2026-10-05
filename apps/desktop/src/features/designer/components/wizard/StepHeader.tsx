import { t, type MessageKey } from '~/shared/i18n';
import { cx } from 'rimstudio-ui';
import type { WizardStep } from './wizard-model';

const STEP_TEXT: Record<WizardStep, MessageKey> = {
  category: 'designer.wizard.step.category',
  describe: 'designer.wizard.step.describe',
  result: 'designer.wizard.step.result',
  name: 'designer.wizard.step.name',
};

/** The name of a step. */
export function stepLabel(step: WizardStep): string {
  return t(STEP_TEXT[step]);
}

export interface StepHeaderProps {
  /** The steps shown, in order (a retune skips the category and name steps). */
  steps: readonly WizardStep[];
  current: WizardStep;
  /** Steps the user may jump to. */
  reachable: (step: WizardStep) => boolean;
  onGo: (step: WizardStep) => void;
}

/** The list of steps with the current one marked; a reachable step can be opened directly. */
export function StepHeader({ steps, current, reachable, onGo }: StepHeaderProps) {
  return (
    <ol class="flex flex-wrap items-center gap-x-4 gap-y-1" aria-label={t('designer.wizard.steps')}>
      {steps.map((step, index) => {
        const active = step === current;
        const enabled = reachable(step);
        return (
          <li key={step} class="flex items-center gap-2">
            <button
              type="button"
              disabled={!enabled}
              aria-current={active ? 'step' : undefined}
              onClick={() => onGo(step)}
              class={cx(
                'inline-flex h-control-sm items-center gap-2 rounded-sm px-1 text-body',
                active ? 'text-fg' : 'text-muted',
                enabled && !active && 'hover:text-fg',
                !enabled && 'cursor-not-allowed opacity-60',
              )}
            >
              <span
                aria-hidden="true"
                class={cx(
                  'inline-flex size-5 items-center justify-center rounded-full border font-mono text-mono-small',
                  active ? 'border-accent bg-accent-tint text-accent' : 'border-line-strong',
                )}
              >
                {index + 1}
              </span>
              <span class={cx(active && 'font-semibold')}>{t(STEP_TEXT[step])}</span>
            </button>
          </li>
        );
      })}
    </ol>
  );
}
