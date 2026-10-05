import { Button } from 'rimstudio-ui';
import { t, type MessageKey } from '~/shared/i18n';

export interface PathOverrideRowProps {
  label: MessageKey;
  /** The path in effect now. */
  value: string | undefined;
  /** True when the value comes from an override, which can then be cleared. */
  overridden: boolean;
  onChoose: () => void;
  onClear: () => void;
}

/** One path with its Choose and Clear override buttons. */
export function PathOverrideRow({
  label,
  value,
  overridden,
  onChoose,
  onClear,
}: PathOverrideRowProps) {
  const name = t(label);
  return (
    <li class="flex flex-wrap items-center gap-2">
      <span class="w-36 shrink-0 text-muted">{name}</span>
      <span class="min-w-0 flex-1 font-mono text-mono break-all">
        {value ?? t('setup.none')}
        {overridden ? <span class="ml-2 text-accent">{t('setup.override.active')}</span> : null}
      </span>
      <Button
        size="sm"
        icon="folder"
        onClick={onChoose}
        aria-label={t('setup.override.choose', { name })}
      >
        {t('setup.override.choose.short')}
      </Button>
      <Button
        size="sm"
        variant="ghost"
        disabled={!overridden}
        onClick={onClear}
        aria-label={t('setup.override.clear', { name })}
      >
        {t('setup.override.clear.short')}
      </Button>
    </li>
  );
}
