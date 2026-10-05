import type { CeCandidateDto, CeChoiceDto } from 'rimstudio-ipc-types';
import { Checkbox, FormField, Select, type SelectOption } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t, tn } from '~/shared/i18n';
import { sourceText } from './labels';

export interface CeChoiceRowProps {
  choice: CeChoiceDto;
  /** What the draft holds now: text for a choice, a boolean for a flag. */
  value: string | boolean | undefined;
  /** The plan takes the derived value of this choice. */
  accepted: boolean;
  onAccept: (on: boolean) => void;
  onChoose: (value: string | boolean | undefined) => void;
}

function candidateText(c: CeCandidateDto): string {
  const used = c.usedBy > 0 ? tn('designer.output.ce.usedBy', c.usedBy) : '';
  const damage =
    c.firstDamage !== undefined
      ? t('designer.output.ce.firstDamage', { value: formatNumber(c.firstDamage, 1) })
      : '';
  const extra = [used, damage].filter((s) => s !== '').join(', ');
  return extra === '' ? c.name : `${c.name} (${extra})`;
}

/**
 * A choice of the block: a def or tag picked from the ranked candidates, or a yes or no flag. The
 * caliber and the class tag are never picked for the user; the candidates come best fit first.
 */
export function CeChoiceRow({ choice, value, accepted, onAccept, onChoose }: CeChoiceRowProps) {
  if (choice.kind === 'flag') {
    return (
      <div data-ce-field={choice.field}>
        <Checkbox checked={value === true} onCheckedChange={(on) => onChoose(on)}>
          {choice.label}
        </Checkbox>
      </div>
    );
  }
  const current = typeof value === 'string' && value !== '' ? value : undefined;
  const options: SelectOption[] = choice.candidates.map((c) => ({
    value: c.name,
    label: candidateText(c),
  }));
  if (current && !options.some((o) => o.value === current)) {
    options.unshift({ value: current, label: current });
  }
  return (
    <div data-ce-field={choice.field} class="flex min-w-0 flex-col gap-1">
      <FormField label={choice.label} required={choice.required && !current}>
        <Select
          value={current}
          options={options}
          placeholder={t('designer.output.ce.choose')}
          onValueChange={(v) => onChoose(v === '' ? undefined : v)}
          disabled={options.length === 0}
        />
      </FormField>
      {!current && choice.reason ? <p class="text-small text-muted">{choice.reason}</p> : null}
      {!current && choice.status === 'derived' && choice.value ? (
        <>
          <Checkbox checked={accepted} onCheckedChange={onAccept}>
            {t('designer.output.ce.use', { value: choice.value })}
          </Checkbox>
          {choice.source ? <p class="text-small text-muted">{sourceText(choice.source)}</p> : null}
        </>
      ) : null}
    </div>
  );
}
