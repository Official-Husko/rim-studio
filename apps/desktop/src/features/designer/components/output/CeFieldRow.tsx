import type { CeSuggestedFieldDto, ValueSourceDto } from 'rimstudio-ipc-types';
import { Badge, Button, Checkbox, Chip, FormField, NumberField } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';
import { ratingText, ratingTone, sourceText } from './labels';

export interface CeFieldRowProps {
  field: CeSuggestedFieldDto;
  /** The plan takes the derived value of this field. */
  accepted: boolean;
  onAccept: (on: boolean) => void;
  /** The user typed, answered or cleared the number. */
  onValue: (value: number | undefined, source: ValueSourceDto) => void;
}

/**
 * One number of the Combat Extended block. A derived value is shown as a hint and taken only when
 * the box is ticked; a number the engine would not guess says why and waits for an answer; a number
 * the draft already holds shows its source and can be cleared.
 */
export function CeFieldRow({ field, accepted, onAccept, onValue }: CeFieldRowProps) {
  const held = field.status === 'held';
  const asks = field.status === 'ask';
  const source: ValueSourceDto = asks ? 'answered' : 'typed';
  return (
    <div data-ce-field={field.field} class="flex min-w-0 flex-col gap-1">
      <FormField label={field.label} required={field.required && !held}>
        <NumberField
          value={held ? field.held : undefined}
          step={0.01}
          {...(field.value !== undefined && !held
            ? { placeholder: formatNumber(field.value, 3), chip: <Chip kind="derived" /> }
            : {})}
          {...(held && field.heldSource
            ? {
                chip: (
                  <Chip kind={field.heldSource.kind === 'typed' ? 'typed' : 'neutral'}>
                    {sourceText(field.heldSource)}
                  </Chip>
                ),
              }
            : {})}
          onValueChange={(value) => onValue(value, source)}
        />
      </FormField>
      <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
        {field.rating && !held ? (
          <Badge tone={ratingTone(field.rating)}>{ratingText(field.rating)}</Badge>
        ) : null}
        {!held && field.source ? (
          <span class="text-small text-muted">{sourceText(field.source)}</span>
        ) : null}
        {held ? (
          <Button size="sm" variant="ghost" onClick={() => onValue(undefined, source)}>
            {t('designer.output.ce.clear')}
          </Button>
        ) : null}
      </div>
      {field.band && field.status === 'derived' ? (
        <span class="font-mono text-mono-small text-faint">
          {t('designer.output.ce.band', {
            p50Low: formatNumber(field.band.p50.low, 3),
            p50High: formatNumber(field.band.p50.high, 3),
            p80Low: formatNumber(field.band.p80.low, 3),
            p80High: formatNumber(field.band.p80.high, 3),
          })}
        </span>
      ) : null}
      {field.status === 'derived' && field.value !== undefined ? (
        <Checkbox checked={accepted} onCheckedChange={onAccept}>
          {t('designer.output.ce.use', { value: formatNumber(field.value, 3) })}
        </Checkbox>
      ) : null}
      {asks && field.reason ? <p class="text-small text-muted">{field.reason}</p> : null}
      {asks && field.reference !== undefined ? (
        <div>
          <Button size="sm" variant="ghost" onClick={() => onValue(field.reference, 'answered')}>
            {t('designer.output.ce.useEstimate', { value: formatNumber(field.reference, 3) })}
          </Button>
        </div>
      ) : null}
    </div>
  );
}
