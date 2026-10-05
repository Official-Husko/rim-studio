import type { ArchetypeChoiceDto } from 'rimstudio-ipc-types';
import { Button } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

export interface ArchetypeBarProps {
  choice: ArchetypeChoiceDto;
  onRetune: () => void;
}

/** A line above the editor for a draft made by the wizard: the type, and a way to propose again. */
export function ArchetypeBar({ choice, onRetune }: ArchetypeBarProps) {
  return (
    <div class="flex flex-wrap items-center justify-between gap-2 border-b border-line bg-surface px-4 py-2">
      <p class="text-small text-muted">
        {t('designer.wizard.bar.text', { type: choice.archetype })}
      </p>
      <Button size="sm" onClick={onRetune}>
        {t('designer.wizard.bar.action')}
      </Button>
    </div>
  );
}
