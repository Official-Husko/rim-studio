import type { CustomIngredientDto, SourcedDto } from 'rimstudio-ipc-types';
import { Button, FormField, IconButton, NumberField, TextField } from 'rimstudio-ui';
import { StringListField } from '~/shared/ce/StringListField';
import { t } from '~/shared/i18n';
import { diagnosticsUnder, typedNumber, typePointer } from './model';
import type { CustomAmmoStore } from './store';

export interface IngredientListProps {
  store: CustomAmmoStore;
  index: number;
}

/** The ingredients of one craft: a thing and a count each, with the things and categories that may stand in. */
export function IngredientList({ store, index }: IngredientListProps) {
  const list: CustomIngredientDto[] = store.custom.value.types[index]?.recipe.ingredients ?? [];
  const pointer = '/recipe/ingredients';
  const save = (next: CustomIngredientDto[]): void => store.setField(index, pointer, next);
  const replace = (at: number, row: CustomIngredientDto | undefined): void => {
    const next = [...list];
    if (row) next[at] = row;
    else next.splice(at, 1);
    save(next);
  };
  return (
    <div
      class="flex flex-col gap-2"
      role="group"
      aria-label={t('ammo.field.ingredients')}
      data-ammo-field={typePointer(index, pointer)}
    >
      <span class="text-small font-semibold text-muted">{t('ammo.field.ingredients')}</span>
      <p class="m-0 text-small text-faint">{t('ammo.field.ingredients-help')}</p>
      {list.map((row, at) => {
        const own = diagnosticsUnder(
          store.diagnostics.value,
          typePointer(index, `${pointer}/${at}`),
        );
        return (
          <div
            key={at}
            data-ammo-field={typePointer(index, `${pointer}/${at}/thing`)}
            class="flex flex-col gap-2 border border-line p-2"
          >
            <div class="flex flex-wrap items-end gap-2">
              <div class="min-w-40 flex-1">
                <FormField label={t('ammo.field.ingredient-thing', { n: at + 1 })}>
                  <TextField
                    value={row.thing}
                    onValueChange={(thing) => replace(at, { ...row, thing })}
                  />
                </FormField>
              </div>
              <div class="w-32" data-ammo-field={typePointer(index, `${pointer}/${at}/count`)}>
                <FormField label={t('ammo.field.ingredient-count', { n: at + 1 })}>
                  <NumberField
                    value={row.count?.value}
                    onValueChange={(n) => {
                      const count: SourcedDto<number> | undefined =
                        n === undefined ? undefined : typedNumber(n);
                      replace(at, {
                        ...row,
                        ...(count ? { count } : { count: undefined }),
                      } as CustomIngredientDto);
                    }}
                    step={1}
                  />
                </FormField>
              </div>
              <IconButton
                icon="trash"
                label={t('ammo.field.ingredient-remove', { n: at + 1 })}
                onClick={() => replace(at, undefined)}
              />
            </div>
            <details>
              <summary class="cursor-pointer text-small text-muted">
                {t('ammo.field.ingredient-more')}
              </summary>
              <div class="mt-2 flex flex-col gap-2">
                <StringListField
                  label={t('ammo.field.alternatives')}
                  values={row.alternatives ?? []}
                  onChange={(alternatives) => replace(at, { ...row, alternatives })}
                />
                <StringListField
                  label={t('ammo.field.ingredient-categories')}
                  values={row.categories ?? []}
                  onChange={(categories) => replace(at, { ...row, categories })}
                />
              </div>
            </details>
            {own.map((d, i) => (
              <p
                key={`${d.code}-${i}`}
                class={
                  d.severity === 'error'
                    ? 'm-0 text-small text-danger'
                    : 'm-0 text-small text-warning'
                }
              >
                {d.message}
              </p>
            ))}
          </div>
        );
      })}
      <div class="self-start">
        <Button size="sm" icon="plus" onClick={() => save([...list, { thing: '' }])}>
          {t('ammo.field.ingredient-add')}
        </Button>
      </div>
    </div>
  );
}
