import { Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { NumberFieldRow } from './NumberFieldRow';
import { RawNodeList } from './RawNodeList';
import { TextFieldRow } from './TextFieldRow';

/** The picture, the icon and the sound of the weapon, and the extra fields of its graphic. */
export function LookPanel() {
  return (
    <Panel title={t('designer.panel.look')} collapsible>
      <div class="flex flex-col gap-4">
        <p class="text-small text-faint">{t('designer.look.help')}</p>
        <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
          <div class="md:col-span-2">
            <TextFieldRow pointer="/texturePath" label={t('designer.field.texture')} />
          </div>
          <TextFieldRow pointer="/graphicClass" label={t('designer.field.graphicClass')} />
          <TextFieldRow
            pointer="/drawSize"
            label={t('designer.field.drawSize')}
            help={t('designer.help.drawSize')}
          />
          <TextFieldRow pointer="/graphicColor" label={t('designer.field.graphicColor')} />
          <TextFieldRow pointer="/soundInteract" label={t('designer.field.soundInteract')} />
          <div class="md:col-span-2">
            <TextFieldRow pointer="/uiIconPath" label={t('designer.field.uiIconPath')} />
          </div>
          <NumberFieldRow
            def={{
              pointer: '/uiIconScale',
              label: 'designer.field.uiIconScale',
              step: 0.05,
              plain: true,
            }}
          />
        </div>
        <RawNodeList
          pointer="/graphicExtra"
          label={t('designer.look.graphicExtra')}
          help={t('designer.look.graphicExtraHelp')}
        />
      </div>
    </Panel>
  );
}
