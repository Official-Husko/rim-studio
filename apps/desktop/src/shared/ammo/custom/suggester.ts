import { batch, type Signal } from '@preact/signals';
import type {
  ApiError,
  CeAmmoSuggestedFieldDto,
  CeAmmoSuggestionDto,
  CustomAmmoDto,
  CustomAmmoTypeDto,
} from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import type { ammoSuggest } from '../api';
import { overlay } from './model';

export interface SuggesterDeps {
  custom: Signal<CustomAmmoDto>;
  ids: Signal<string[]>;
  suggestions: Signal<Readonly<Record<string, CeAmmoSuggestionDto>>>;
  suggesting: Signal<string | undefined>;
  suggestError: Signal<ApiError | undefined>;
  suggest: typeof ammoSuggest;
  updateType: (index: number, change: (type: CustomAmmoTypeDto) => CustomAmmoTypeDto) => void;
  setField: (index: number, pointer: string, value: unknown) => void;
}

/**
 * Asking the backend for the values of a type: a suggestion from the nearest of the user's own ammunition,
 * or a copy of an existing type. The answers are kept per type (by its id) so the form can show the source
 * and the rating of each field.
 */
export function createSuggester(deps: SuggesterDeps) {
  const { custom, ids, suggestions, suggesting, suggestError, suggest, updateType, setField } =
    deps;

  async function suggestFor(index: number, options: { copyFrom?: string } = {}): Promise<boolean> {
    const type = custom.peek().types[index];
    const id = ids.peek()[index];
    if (!type || id === undefined || type.ammoClass.trim() === '') return false;
    suggesting.value = id;
    suggestError.value = undefined;
    try {
      const whole = custom.peek();
      const result = await suggest({
        class: type.ammoClass,
        hints: {
          ...(whole.caliber.trim() !== '' ? { caliber: whole.caliber } : {}),
          ...(type.projectile.damage ? { damage: type.projectile.damage.value } : {}),
          ...(type.projectile.speed ? { speed: type.projectile.speed.value } : {}),
          ...(whole.similarTo ? { similarSet: whole.similarTo } : {}),
        },
        ...(options.copyFrom ? { copyFrom: options.copyFrom } : {}),
      });
      const at = ids.peek().indexOf(id);
      if (at < 0) return false;
      batch(() => {
        suggestions.value = { ...suggestions.peek(), [id]: result };
        if (result.available) {
          updateType(at, (current) => {
            const base = result.ammoType;
            if (options.copyFrom) {
              const copied = structuredClone(base);
              copied.key = current.key.trim() !== '' ? current.key : base.key;
              copied.copiedFrom = options.copyFrom;
              return copied;
            }
            const merged = overlay(base, current) as CustomAmmoTypeDto;
            return { ...merged, key: current.key.trim() !== '' ? current.key : base.key };
          });
        }
      });
      return result.available;
    } catch (thrown) {
      suggestError.value = normalizeError(thrown);
      return false;
    } finally {
      suggesting.value = undefined;
    }
  }

  /** The suggestion the backend gave for one field of one type, if any. */
  function suggestionFor(index: number, pointer: string): CeAmmoSuggestedFieldDto | undefined {
    const id = ids.value[index];
    return id === undefined
      ? undefined
      : suggestions.value[id]?.fields.find((f) => f.field === pointer);
  }

  /** Put the suggested value of a field into the type as a suggested number or text. */
  function applySuggestion(index: number, pointer: string): void {
    const field = suggestionFor(index, pointer);
    if (!field) return;
    if (field.value !== undefined) {
      setField(index, pointer, { value: field.value, source: 'suggested' });
    } else if (field.text !== undefined) {
      setField(index, pointer, field.text);
    }
  }

  return { suggestFor, suggestionFor, applySuggestion };
}
