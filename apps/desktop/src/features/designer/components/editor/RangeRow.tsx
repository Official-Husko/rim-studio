import { useEffect, useState } from 'preact/hooks';
import type { FloatRangeDto } from 'rimstudio-ipc-types';
import { FormField, NumberField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { rangeOf } from '../../model/assets';
import { diagnosticsFor } from '../../model/draft';
import type { Pointer } from '../../model/pointer';
import { AssetProblems } from './AssetProblems';
import { useFieldEnv } from './fieldEnv';

export interface RangeRowProps {
  pointer: Pointer;
  label: string;
  help?: string;
  step: number;
  value: FloatRangeDto | undefined;
  onChange: (value: FloatRangeDto | undefined) => void;
}

/** A lowest and a highest number. The range is written once both are there; empty removes it. */
export function RangeRow({ pointer, label, help, step, value, onChange }: RangeRowProps) {
  const env = useFieldEnv();
  const [min, setMin] = useState(value?.min);
  const [max, setMax] = useState(value?.max);
  useEffect(() => {
    setMin(value?.min);
    setMax(value?.max);
  }, [value?.min, value?.max]);

  const commit = (nextMin: number | undefined, nextMax: number | undefined): void => {
    setMin(nextMin);
    setMax(nextMax);
    const range = rangeOf(nextMin, nextMax);
    if (range !== 'incomplete') onChange(range);
  };
  const incomplete = rangeOf(min, max) === 'incomplete';

  return (
    <fieldset data-field={pointer} class="flex min-w-0 flex-col gap-1">
      <legend class="mb-1 text-small font-semibold text-muted">{label}</legend>
      <div class="grid grid-cols-2 gap-2">
        <FormField label={t('designer.sounds.lowest')}>
          <NumberField
            value={min}
            step={step}
            aria-label={t('designer.sounds.lowestOf', { field: label })}
            onValueChange={(v) => commit(v, max)}
          />
        </FormField>
        <FormField label={t('designer.sounds.highest')}>
          <NumberField
            value={max}
            step={step}
            aria-label={t('designer.sounds.highestOf', { field: label })}
            onValueChange={(v) => commit(min, v)}
          />
        </FormField>
      </div>
      {incomplete ? <p class="text-small text-warning">{t('designer.sounds.bothEnds')}</p> : null}
      {help ? <p class="text-small text-faint">{help}</p> : null}
      <AssetProblems diagnostics={diagnosticsFor(env.diagnostics, pointer)} />
    </fieldset>
  );
}
