import { Button } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { getAt } from '../../model/pointer';
import { ChipListField } from './ChipListField';
import { useFieldEnv } from './fieldEnv';
import { NumberFieldRow } from './NumberFieldRow';
import { NumberMapField } from './NumberMapField';
import { RawNodeList } from './RawNodeList';
import { TextFieldRow } from './TextFieldRow';

/** The recipe of the weapon: who can make it, with which skill levels, and the other recipe fields. */
export function RecipeFields() {
  const env = useFieldEnv();
  const recipe = getAt(env.spec, '/recipe');
  const attrs = Object.entries(env.spec.recipe?.attrs ?? {});
  if (recipe === undefined) {
    return (
      <div data-field="/recipe" class="flex flex-col gap-1">
        <h3 class="font-display text-label tracking-label text-muted uppercase">
          {t('designer.recipe.title')}
        </h3>
        <p class="text-small text-muted">{t('designer.recipe.none')}</p>
        <div>
          <Button size="sm" icon="plus" onClick={() => env.setField('/recipe', {})}>
            {t('designer.recipe.add')}
          </Button>
        </div>
      </div>
    );
  }
  return (
    <div data-field="/recipe" class="flex flex-col gap-3 border-t border-line-subtle pt-3">
      <div class="flex items-center justify-between gap-2">
        <h3 class="font-display text-label tracking-label text-muted uppercase">
          {t('designer.recipe.title')}
        </h3>
        <Button size="sm" variant="ghost" onClick={() => env.setField('/recipe', undefined)}>
          {t('designer.recipe.remove')}
        </Button>
      </div>
      <NumberMapField
        pointer="/recipe/skillRequirements"
        label={t('designer.recipe.skills')}
        help={t('designer.recipe.skillsHelp')}
        keyLabel={t('designer.recipe.skill')}
        valueLabel={t('designer.recipe.level')}
        addLabel={t('designer.recipe.addSkill')}
        removeLabel={(name) => t('designer.recipe.removeSkill', { name })}
      />
      <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
        <TextFieldRow pointer="/recipe/workSkill" label={t('designer.recipe.workSkill')} />
        <NumberFieldRow
          def={{
            pointer: '/recipe/displayPriority',
            label: 'designer.recipe.displayPriority',
            step: 1,
            plain: true,
          }}
        />
        <TextFieldRow
          pointer="/recipe/unfinishedThingDef"
          label={t('designer.recipe.unfinished')}
        />
        <ChipListField pointer="/recipe/recipeUsers" label={t('designer.recipe.users')} />
      </div>
      <RawNodeList
        pointer="/recipe/extra"
        label={t('designer.recipe.otherFields')}
        help={t('designer.recipe.otherFieldsHelp')}
      />
      {attrs.length > 0 ? (
        <p class="font-mono text-mono-small text-faint">
          {t('designer.recipe.attrs', {
            attrs: attrs.map(([k, v]) => `${k}="${v}"`).join(' '),
          })}
        </p>
      ) : null}
    </div>
  );
}
