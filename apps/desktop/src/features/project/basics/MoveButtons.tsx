import { Button, Icon } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

export interface MoveButtonsProps {
  /** What is moved, for the accessible names ("Combat Extended"). */
  name: string;
  canUp: boolean;
  canDown: boolean;
  onUp: () => void;
  onDown: () => void;
  disabled?: boolean;
}

/** Two small buttons that move a row up or down in its list. */
export function MoveButtons({ name, canUp, canDown, onUp, onDown, disabled }: MoveButtonsProps) {
  return (
    <span class="inline-flex">
      <Button
        size="sm"
        variant="ghost"
        aria-label={t('project.basics.move.up', { name })}
        disabled={disabled || !canUp}
        onClick={onUp}
      >
        <Icon name="chevron" turn={3} />
      </Button>
      <Button
        size="sm"
        variant="ghost"
        aria-label={t('project.basics.move.down', { name })}
        disabled={disabled || !canDown}
        onClick={onDown}
      >
        <Icon name="chevron" turn={1} />
      </Button>
    </span>
  );
}
