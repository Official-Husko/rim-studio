import type { CustomAmmoRecipeDto } from 'rimstudio-ipc-types';
import { Group } from '~/shared/ce/Group';
import { RawNodeListField } from '~/shared/ce/RawNodeListField';
import { StringListField } from '~/shared/ce/StringListField';
import { t } from '~/shared/i18n';
import { AmmoNumberField, AmmoTextField } from './fields';
import { IngredientList } from './IngredientList';
import type { CustomAmmoStore } from './store';

export interface RecipeFieldsProps {
  store: CustomAmmoStore;
  index: number;
}

/** The recipe that makes the ammunition: ingredients, work, the workbenches and the research that unlocks it. */
export function RecipeFields({ store, index }: RecipeFieldsProps) {
  const recipe: CustomAmmoRecipeDto = store.custom.value.types[index]?.recipe ?? {};
  const field = { store, index } as const;
  return (
    <div class="flex flex-col gap-3">
      <Group title={t('ammo.recipe.craft')} open>
        <IngredientList store={store} index={index} />
        <div class="grid grid-cols-[repeat(auto-fit,minmax(14rem,1fr))] gap-x-4 gap-y-3">
          <AmmoNumberField
            {...field}
            pointer="/recipe/products"
            label={t('ammo.field.products')}
            help={t('ammo.field.products-help')}
            whole
            step={10}
          />
          <AmmoNumberField
            {...field}
            pointer="/recipe/workAmount"
            label={t('ammo.field.work')}
            unit={t('ammo.unit.ticks')}
            step={50}
          />
          <AmmoNumberField
            {...field}
            pointer="/recipe/skillLevel"
            label={t('ammo.field.skill')}
            whole
            step={1}
          />
        </div>
      </Group>
      <Group title={t('ammo.recipe.where')} summary={t('ammo.recipe.where-help')}>
        <StringListField
          label={t('ammo.field.users')}
          help={t('ammo.field.users-help')}
          values={recipe.users ?? []}
          onChange={(v) => store.setField(index, '/recipe/users', v)}
        />
        <AmmoTextField
          {...field}
          pointer="/recipe/researchPrerequisite"
          label={t('ammo.field.research')}
          help={t('ammo.field.research-help')}
        />
        <StringListField
          label={t('ammo.field.research-all')}
          values={recipe.researchPrerequisites ?? []}
          onChange={(v) => store.setField(index, '/recipe/researchPrerequisites', v)}
        />
      </Group>
      <Group title={t('ammo.recipe.text')} summary={t('ammo.recipe.text-help')}>
        <div class="grid grid-cols-[repeat(auto-fit,minmax(14rem,1fr))] gap-x-4 gap-y-3">
          <AmmoTextField
            {...field}
            pointer="/recipe/parent"
            label={t('ammo.field.parent')}
            help={t('ammo.field.recipe-parent-help')}
          />
          <AmmoTextField {...field} pointer="/recipe/label" label={t('ammo.field.recipe-label')} />
          <AmmoTextField {...field} pointer="/recipe/jobString" label={t('ammo.field.job')} />
        </div>
        <AmmoTextField
          {...field}
          pointer="/recipe/description"
          label={t('ammo.field.recipe-description')}
          multiline
        />
      </Group>
      <Group title={t('ammo.raw.title')} summary={t('ammo.raw.summary')}>
        <RawNodeListField
          label={t('ammo.raw.recipe')}
          help={t('ammo.raw.help')}
          nodes={recipe.extra ?? []}
          onChange={(nodes) => store.setField(index, '/recipe/extra', nodes)}
        />
      </Group>
    </div>
  );
}
