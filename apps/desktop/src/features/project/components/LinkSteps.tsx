import { t } from '~/shared/i18n';

export interface LinkStepsProps {
  /** The mod's name as the game lists it. */
  name: string;
  /** The project has a Combat Extended patch. */
  hasCePatch: boolean;
}

/** The short steps to find the mod in the game. */
export function LinkSteps({ name, hasCePatch }: LinkStepsProps) {
  const steps = [
    t('project.link.steps.start'),
    t('project.link.steps.enable', { name }),
    ...(hasCePatch ? [t('project.link.steps.ce', { name })] : []),
    t('project.link.steps.find'),
  ];
  return (
    <details class="border border-line-subtle bg-raised">
      <summary class="cursor-pointer px-3 py-2 text-body font-semibold text-fg">
        {t('project.link.steps.title')}
      </summary>
      <ol class="m-0 flex flex-col gap-1 px-8 pb-3 pt-1 text-body text-fg">
        {steps.map((step) => (
          <li key={step}>{step}</li>
        ))}
      </ol>
    </details>
  );
}
