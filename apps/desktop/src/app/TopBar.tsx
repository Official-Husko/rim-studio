import type { ComponentChildren } from 'preact';
import { Badge, Button, Icon } from 'rimstudio-ui';
import { runningCount } from '~/shared/ipc';
import { t, tn } from '~/shared/i18n';
import { ConnectionChip } from './ConnectionChip';

export interface TopBarProps {
  /** The project selector slot; a later task fills it. */
  projectSlot?: ComponentChildren;
  tasksOpen: boolean;
  onToggleTasks: () => void;
}

/** The top bar: product name, project selector slot, connection chip and the task centre button. */
export function TopBar({ projectSlot, tasksOpen, onToggleTasks }: TopBarProps) {
  const running = runningCount.value;
  return (
    <header class="flex h-topbar shrink-0 items-center gap-4 border-b border-line bg-surface px-4">
      <span class="font-display text-title font-semibold tracking-display">{t('app.name')}</span>
      <div
        class="flex min-w-0 flex-1 items-center"
        aria-label={t('project.slot.label')}
        role="group"
      >
        {projectSlot ?? (
          <Button size="sm" disabled icon="folder">
            {t('project.slot.none')}
          </Button>
        )}
      </div>
      <ConnectionChip />
      <Button
        size="sm"
        variant={tasksOpen ? 'primary' : 'secondary'}
        aria-expanded={tasksOpen ? 'true' : 'false'}
        aria-label={
          running > 0 ? `${t('tasks.open')}, ${tn('tasks.running', running)}` : t('tasks.open')
        }
        onClick={onToggleTasks}
      >
        <Icon name="tasks" />
        {t('tasks.open')}
        {running > 0 ? <Badge tone="info">{running}</Badge> : null}
      </Button>
    </header>
  );
}
