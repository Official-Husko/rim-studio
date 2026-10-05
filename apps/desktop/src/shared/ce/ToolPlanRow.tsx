import type { CeToolPlanDto } from 'rimstudio-ipc-types';
import { Button, FormField, IconButton, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { PlainNumberField } from './PlainNumberField';
import { StringListField } from './StringListField';

export interface ToolPlanRowProps {
  tool: CeToolPlanDto;
  index: number;
  count: number;
  onChange: (tool: CeToolPlanDto) => void;
  onRemove: () => void;
  onMove: (delta: -1 | 1) => void;
}

type NumberKey =
  'power' | 'cooldown' | 'chanceFactor' | 'armorPenetrationSharp' | 'armorPenetrationBlunt';

/** One tool of the converted weapon: where it comes from and the Combat Extended fields it sets. */
export function ToolPlanRow({ tool, index, count, onChange, onRemove, onMove }: ToolPlanRowProps) {
  const name = tool.label === '' ? t('ceblock.tools.unnamed') : tool.label;
  const number = (key: NumberKey, value: number | undefined): void => {
    const next = { ...tool };
    if (value === undefined) delete next[key];
    else next[key] = value;
    onChange(next);
  };
  const text = (key: 'from' | 'linkedBodyPartsGroup', value: string): void => {
    const next = { ...tool };
    if (value === '') delete next[key];
    else next[key] = value;
    onChange(next);
  };
  return (
    <div
      class="flex flex-col gap-3 rounded-sm border border-line p-3"
      role="group"
      aria-label={name}
    >
      <div class="flex items-end gap-2">
        <div class="min-w-0 flex-1">
          <FormField label={t('ceblock.tools.label')} help={t('ceblock.tools.labelHelp')}>
            <TextField
              aria-label={t('ceblock.tools.label')}
              value={tool.label}
              onValueChange={(label) => onChange({ ...tool, label })}
            />
          </FormField>
        </div>
        <Button
          size="sm"
          variant="secondary"
          aria-label={t('ceblock.tools.up', { name })}
          disabled={index === 0}
          onClick={() => onMove(-1)}
        >
          {t('ceblock.tools.upShort')}
        </Button>
        <Button
          size="sm"
          variant="secondary"
          aria-label={t('ceblock.tools.down', { name })}
          disabled={index === count - 1}
          onClick={() => onMove(1)}
        >
          {t('ceblock.tools.downShort')}
        </Button>
        <IconButton icon="trash" label={t('ceblock.tools.remove', { name })} onClick={onRemove} />
      </div>
      <div class="grid grid-cols-2 gap-3">
        <FormField label={t('ceblock.tools.from')} help={t('ceblock.tools.fromHelp')}>
          <TextField
            aria-label={t('ceblock.tools.from')}
            value={tool.from ?? ''}
            onValueChange={(v) => text('from', v)}
          />
        </FormField>
        <FormField label={t('ceblock.tools.group')}>
          <TextField
            aria-label={t('ceblock.tools.group')}
            value={tool.linkedBodyPartsGroup ?? ''}
            onValueChange={(v) => text('linkedBodyPartsGroup', v)}
          />
        </FormField>
      </div>
      <StringListField
        label={t('ceblock.tools.capacities')}
        help={t('ceblock.tools.capacitiesHelp')}
        values={tool.capacities ?? []}
        onChange={(capacities) => {
          const next = { ...tool };
          if (capacities.length === 0) delete next.capacities;
          else next.capacities = capacities;
          onChange(next);
        }}
      />
      <div class="grid grid-cols-2 gap-3 sm:grid-cols-3">
        <PlainNumberField
          label={t('ceblock.tools.power')}
          value={tool.power}
          onChange={(v) => number('power', v)}
        />
        <PlainNumberField
          label={t('ceblock.tools.cooldown')}
          unit="s"
          value={tool.cooldown}
          onChange={(v) => number('cooldown', v)}
        />
        <PlainNumberField
          label={t('ceblock.tools.chance')}
          value={tool.chanceFactor}
          onChange={(v) => number('chanceFactor', v)}
        />
        <PlainNumberField
          label={t('ceblock.tools.sharp')}
          value={tool.armorPenetrationSharp}
          onChange={(v) => number('armorPenetrationSharp', v)}
        />
        <PlainNumberField
          label={t('ceblock.tools.blunt')}
          value={tool.armorPenetrationBlunt}
          onChange={(v) => number('armorPenetrationBlunt', v)}
        />
      </div>
    </div>
  );
}
