import { Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { ChipListField } from './ChipListField';

/** Weapon tags, trade tags and weapon classes, each as a list of chips. */
export function TagsPanel() {
  return (
    <Panel title={t('designer.panel.tags')} collapsible>
      <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
        <ChipListField
          pointer="/weaponTags"
          label={t('designer.field.weaponTags')}
          help={t('designer.help.weaponTags')}
        />
        <ChipListField
          pointer="/tradeTags"
          label={t('designer.field.tradeTags')}
          help={t('designer.help.tradeTags')}
        />
        <div class="md:col-span-2">
          <ChipListField
            pointer="/weaponClasses"
            label={t('designer.field.weaponClasses')}
            help={t('designer.help.weaponClasses')}
          />
        </div>
      </div>
    </Panel>
  );
}
