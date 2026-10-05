import type { SourcedDto } from 'rimstudio-ipc-types';
import { Chip, FormField, NumberField, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { chipKindOf, typedNumber, typePointer } from './model';
import { FieldNotes } from './FieldNotes';
import type { CustomAmmoStore } from './store';

export interface FieldProps {
  store: CustomAmmoStore;
  /** The index of the type being edited. */
  index: number;
  /** The pointer of the field inside the type, such as `/projectile/damage`. */
  pointer: string;
  label: string;
  help?: string;
  unit?: string;
}

function chipText(source: SourcedDto<number>['source']): string {
  switch (source) {
    case 'typed':
      return t('ammo.source.typed');
    case 'suggested':
      return t('ammo.source.suggested');
    case 'anchor':
      return t('ammo.source.anchor');
    default:
      return t('ammo.source.given');
  }
}

/** The diagnostics of a field of a type, read from the plan. */
function notesOf(store: CustomAmmoStore, index: number, pointer: string) {
  const list = store.diagnostics.value.filter((d) => d.field === typePointer(index, pointer));
  return { error: list.find((d) => d.severity === 'error')?.message, list };
}

/** A number with its source chip, its unit, the suggestion of the backend and the checks of the plan. */
export function AmmoNumberField({
  store,
  index,
  pointer,
  label,
  help,
  unit,
  whole,
  step,
}: FieldProps & { whole?: boolean; step?: number }) {
  const value = store.valueAt(index, pointer) as SourcedDto<number> | undefined;
  const { error, list } = notesOf(store, index, pointer);
  return (
    <div data-ammo-field={typePointer(index, pointer)} class="flex flex-col gap-1">
      <FormField label={label} {...(help ? { help } : {})} {...(error ? { error } : {})}>
        <div class="w-48">
          <NumberField
            aria-label={label}
            value={value?.value}
            onValueChange={(n) =>
              store.setField(
                index,
                pointer,
                n === undefined ? undefined : typedNumber(whole ? Math.max(0, Math.round(n)) : n),
              )
            }
            step={step ?? (whole ? 1 : 0.01)}
            invalid={error !== undefined}
            {...(unit ? { unit } : {})}
            {...(value
              ? { chip: <Chip kind={chipKindOf(value.source)}>{chipText(value.source)}</Chip> }
              : {})}
          />
        </div>
      </FormField>
      <FieldNotes
        suggestion={store.suggestionFor(index, pointer)}
        current={value?.value}
        unit={unit}
        onUse={() => store.applySuggestion(index, pointer)}
        diagnostics={list}
      />
    </div>
  );
}

/** A text of the type with the suggestion of the backend and the checks of the plan. */
export function AmmoTextField({
  store,
  index,
  pointer,
  label,
  help,
  multiline,
}: FieldProps & { multiline?: boolean }) {
  const value = store.valueAt(index, pointer);
  const text = typeof value === 'string' ? value : '';
  const { error, list } = notesOf(store, index, pointer);
  return (
    <div data-ammo-field={typePointer(index, pointer)} class="flex flex-col gap-1">
      <FormField label={label} {...(help ? { help } : {})} {...(error ? { error } : {})}>
        <TextField
          value={text}
          onValueChange={(v) => store.setField(index, pointer, v === '' ? undefined : v)}
          invalid={error !== undefined}
          {...(multiline ? { multiline: true, rows: 2 } : {})}
        />
      </FormField>
      <FieldNotes
        suggestion={store.suggestionFor(index, pointer)}
        current={text}
        onUse={() => store.applySuggestion(index, pointer)}
        diagnostics={list}
      />
    </div>
  );
}
