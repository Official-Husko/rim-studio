import type { ArchetypeCatalogDto, BalanceTargetDto } from 'rimstudio-ipc-types';
import { SegmentedControl } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

export interface BalanceControlProps {
  catalog: ArchetypeCatalogDto;
  value: BalanceTargetDto;
  onChange: (target: BalanceTargetDto) => void;
}

const KEYS = {
  weaker: 'designer.wizard.balance.weaker',
  typical: 'designer.wizard.balance.typical',
  stronger: 'designer.wizard.balance.stronger',
} as const;

/** Weaker, typical or stronger than most weapons of the class; the catalogue says what each means. */
export function BalanceControl({ catalog, value, onChange }: BalanceControlProps) {
  const options = catalog.balance.flatMap((option) =>
    typeof option.target === 'string'
      ? [{ value: option.target, label: t(KEYS[option.target]), full: option.label }]
      : [],
  );
  const current = typeof value === 'string' ? value : '';
  const help = options.find((o) => o.value === current)?.full;
  return (
    <div class="flex flex-col gap-1">
      <SegmentedControl
        label={t('designer.wizard.balance.label')}
        options={options.map(({ value: v, label }) => ({ value: v, label }))}
        value={current}
        onValueChange={(next) => {
          const found = options.find((o) => o.value === next);
          if (found) onChange(found.value);
        }}
      />
      {help ? <p class="text-small text-muted">{help}</p> : null}
    </div>
  );
}
