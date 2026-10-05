import type { AskItemDto } from 'rimstudio-ipc-types';
import {
  Chip,
  FormField,
  NumberField,
  SegmentedControl,
  Button,
  Combobox,
  type ComboboxOption,
} from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';
import type { AnswerScope } from './answerStore';

export interface AskRowProps {
  ask: AskItemDto;
  value: string | number | boolean | undefined;
  /** Where an answer in force came from, when it was not given for this weapon. */
  from: AnswerScope | undefined;
  /** Options of a choice with the ranked ones first and a hint on each. */
  options: ComboboxOption[];
  /** A short line on how the choices were ranked. */
  rankNote?: string;
  onChange: (value: string | number | boolean | undefined) => void;
}

/** One open question of a weapon: the label, the reason it is asked and the control for the answer. */
export function AskRow({ ask, value, from, options, rankNote, onChange }: AskRowProps) {
  const id = `ask-${ask.field.replace(/[^a-z0-9]+/gi, '-')}`;
  const shared =
    from === 'family' ? <Chip kind="derived">{t('patches.ask.from-family')}</Chip> : null;
  return (
    <FormField label={ask.label} {...(rankNote ? { help: rankNote } : {})}>
      <div class="flex flex-col gap-2">
        {ask.kind === 'choice' ? (
          <Combobox
            id={id}
            aria-label={ask.label}
            options={options}
            value={typeof value === 'string' ? value : undefined}
            onValueChange={onChange}
            placeholder={t('patches.ask.choose')}
            emptyText={t('patches.ask.no-match')}
          />
        ) : null}
        {ask.kind === 'flag' ? (
          <div class="self-start">
            <SegmentedControl
              label={ask.label}
              value={typeof value === 'boolean' ? (value ? 'yes' : 'no') : ''}
              onValueChange={(v) => onChange(v === 'yes')}
              options={[
                { value: 'yes', label: t('patches.ask.yes') },
                { value: 'no', label: t('patches.ask.no') },
              ]}
            />
          </div>
        ) : null}
        {ask.kind === 'number' ? (
          <div class="flex flex-wrap items-center gap-2">
            <div class="w-40">
              <NumberField
                id={id}
                aria-label={ask.label}
                value={typeof value === 'number' ? value : undefined}
                onValueChange={onChange}
                step={0.01}
                {...(ask.suggestion !== undefined
                  ? { placeholder: formatNumber(ask.suggestion, 3) }
                  : {})}
              />
            </div>
            {ask.suggestion !== undefined && value === undefined ? (
              <Button size="sm" variant="secondary" onClick={() => onChange(ask.suggestion)}>
                {t('patches.ask.use-estimate', { value: formatNumber(ask.suggestion, 3) })}
              </Button>
            ) : null}
          </div>
        ) : null}
        {shared}
        {ask.reason && value === undefined ? (
          <p class="m-0 text-small text-muted">{ask.reason}</p>
        ) : null}
      </div>
    </FormField>
  );
}
