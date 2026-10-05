import type { SourcedDto } from 'rimstudio-ipc-types';
import { Button, FormField, NumberField } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';
import { diagnosticsFor, suggested, typed } from '../../model/draft';
import type { NumberFieldDef } from '../../model/fields';
import { getAt } from '../../model/pointer';
import { DiagnosticNotes } from './DiagnosticNotes';
import { useFieldEnv } from './fieldEnv';
import { SourceChip } from './SourceChip';

export interface NumberFieldRowProps {
  def: NumberFieldDef;
  /** Overrides the label of the definition (a tool field prefixed by the tool name). */
  labelText?: string;
}

/**
 * A number of the draft with its unit, its source chip, the pool range, the suggestion and the
 * diagnostics of the field. Typing makes the value typed; a suggestion is only used on request.
 */
export function NumberFieldRow({ def, labelText }: NumberFieldRowProps) {
  const env = useFieldEnv();
  const stored = getAt(env.spec, def.pointer);
  const current: SourcedDto<number> | undefined = def.plain
    ? typeof stored === 'number'
      ? { value: stored, source: 'typed' }
      : undefined
    : (stored as SourcedDto<number> | undefined);
  const suggestion = env.suggestions.get(def.pointer);
  const pool = suggestion ? env.pools.get(suggestion.stat) : undefined;
  const diagnostics = diagnosticsFor(env.diagnostics, def.pointer);
  const errors = diagnostics.filter((d) => d.severity === 'error').map((d) => d.message);
  const offered =
    suggestion?.value !== undefined && !suggestion.locked && suggestion.value !== current?.value
      ? suggestion.value
      : undefined;
  return (
    <div data-field={def.pointer} class="flex min-w-0 flex-col gap-1">
      <FormField label={labelText ?? t(def.label)} error={errors.join(' ') || undefined}>
        <NumberField
          value={current?.value}
          step={def.step}
          {...(def.unit ? { unit: def.unit } : {})}
          {...(offered !== undefined ? { placeholder: formatNumber(offered, 3) } : {})}
          {...(current && !def.plain ? { chip: <SourceChip source={current.source} /> } : {})}
          onValueChange={(value) =>
            env.setField(
              def.pointer,
              value === undefined ? undefined : def.plain ? value : typed(value),
            )
          }
        />
      </FormField>
      {pool || offered !== undefined ? (
        <div class="flex flex-wrap items-center gap-x-3 gap-y-1 font-mono text-mono-small text-faint">
          {pool ? (
            <span>
              {t('designer.hint.pool', {
                p10: formatNumber(pool.p10, 3),
                median: formatNumber(pool.median, 3),
                p90: formatNumber(pool.p90, 3),
              })}
            </span>
          ) : null}
          {offered !== undefined ? (
            <Button
              size="sm"
              variant="ghost"
              onClick={() => env.setField(def.pointer, suggested(offered))}
            >
              {t('designer.hint.use', { value: formatNumber(offered, 3) })}
            </Button>
          ) : null}
        </div>
      ) : null}
      <DiagnosticNotes diagnostics={diagnostics} />
    </div>
  );
}
