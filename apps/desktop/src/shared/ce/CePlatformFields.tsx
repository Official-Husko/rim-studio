import { Button, Switch } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { AttachmentLinkRow } from './AttachmentLinkRow';
import { newLink, replaceAt, type CeBlock, type CeBlockPatch } from './blockModel';
import { GraphicPartRow } from './GraphicPartRow';
import { Group } from './Group';

export interface CePlatformFieldsProps {
  block: CeBlock;
  onChange: (patch: CeBlockPatch) => void;
}

/** The weapon platform parameters: the flag, the attachments the weapon accepts and its default parts. */
export function CePlatformFields({ block, onChange }: CePlatformFieldsProps) {
  const links = block.attachmentLinks ?? [];
  const parts = block.defaultGraphicParts ?? [];
  const on = block.isWeaponPlatform === true;
  const summary = on
    ? [tn('ceblock.platform.links', links.length), tn('ceblock.platform.parts', parts.length)].join(
        ', ',
      )
    : t('ceblock.off');
  return (
    <Group title={t('ceblock.platform.title')} summary={summary}>
      <p class="m-0 text-small text-muted">{t('ceblock.platform.help')}</p>
      <Switch checked={on} onCheckedChange={(v) => onChange({ isWeaponPlatform: v })}>
        {t('ceblock.platform.flag')}
      </Switch>
      <div class="flex flex-col gap-2">
        {links.map((link, index) => (
          <AttachmentLinkRow
            key={index}
            link={link}
            onChange={(next) => onChange({ attachmentLinks: replaceAt(links, index, next) })}
            onRemove={() => onChange({ attachmentLinks: replaceAt(links, index, undefined) })}
          />
        ))}
        <div class="self-start">
          <Button
            size="sm"
            variant="secondary"
            onClick={() => onChange({ attachmentLinks: [...links, newLink()] })}
          >
            {t('ceblock.platform.addLink')}
          </Button>
        </div>
      </div>
      <div class="flex flex-col gap-2">
        {parts.map((part, index) => (
          <GraphicPartRow
            key={index}
            part={part}
            onChange={(next) => onChange({ defaultGraphicParts: replaceAt(parts, index, next) })}
            onRemove={() => onChange({ defaultGraphicParts: replaceAt(parts, index, undefined) })}
          />
        ))}
        <div class="self-start">
          <Button
            size="sm"
            variant="secondary"
            onClick={() => onChange({ defaultGraphicParts: [...parts, {}] })}
          >
            {t('ceblock.platform.addPart')}
          </Button>
        </div>
      </div>
    </Group>
  );
}
