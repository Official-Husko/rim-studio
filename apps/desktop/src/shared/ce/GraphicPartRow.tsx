import type { CeGraphicPartDto } from 'rimstudio-ipc-types';
import { Button } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { RawNodeField } from './RawNodeField';
import { StringListField } from './StringListField';

export interface GraphicPartRowProps {
  part: CeGraphicPartDto;
  onChange: (part: CeGraphicPartDto) => void;
  onRemove: () => void;
}

/** One default graphic part of a platform: the part graphic, its outline and the slot tags it fits. */
export function GraphicPartRow({ part, onChange, onRemove }: GraphicPartRowProps) {
  const node = (
    key: 'partGraphic' | 'outlineGraphic',
    value: CeGraphicPartDto[typeof key],
  ): void => {
    const next = { ...part };
    if (value === undefined) delete next[key];
    else next[key] = value;
    onChange(next);
  };
  return (
    <div class="flex flex-col gap-3 rounded-sm border border-line p-3">
      <StringListField
        label={t('ceblock.part.slots')}
        help={t('ceblock.part.slotsHelp')}
        values={part.slotTags ?? []}
        onChange={(slotTags) => {
          const next = { ...part };
          if (slotTags.length === 0) delete next.slotTags;
          else next.slotTags = slotTags;
          onChange(next);
        }}
      />
      <RawNodeField
        label={t('ceblock.part.graphic')}
        node={part.partGraphic}
        onChange={(v) => node('partGraphic', v)}
      />
      <RawNodeField
        label={t('ceblock.part.outline')}
        node={part.outlineGraphic}
        onChange={(v) => node('outlineGraphic', v)}
      />
      <div class="self-start">
        <Button size="sm" variant="secondary" onClick={onRemove}>
          {t('ceblock.part.remove')}
        </Button>
      </div>
    </div>
  );
}
