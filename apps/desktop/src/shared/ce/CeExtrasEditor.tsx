import type { CeOptionDto } from 'rimstudio-ipc-types';
import type { ComboboxOption } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import type { CeBlock, CeBlockPatch } from './blockModel';
import { CeBowFields } from './CeBowFields';
import { CeOptionsList } from './CeOptionsList';
import { CePlatformFields } from './CePlatformFields';
import { CeToolPlanFields } from './CeToolPlanFields';
import { CeUnderBarrelFields } from './CeUnderBarrelFields';
import { Group } from './Group';
import { RawNodeListField } from './RawNodeListField';
import { StringListField } from './StringListField';

export interface CeExtrasEditorProps {
  /** The Combat Extended block being edited (the draft's, or the answers of a conversion). */
  block: CeBlock;
  /** Called with the members that changed; an undefined member is removed from the block. */
  onChange: (patch: CeBlockPatch) => void;
  /** What the user's own conversions suggest. Optional: nothing is taken without a click. */
  options?: readonly CeOptionDto[];
  /** Ammo sets the under barrel unit can pick from. */
  ammoSets?: ComboboxOption[];
}

/**
 * The optional parts of the Combat Extended block beyond the plain numbers: bows, ammo extras, companion
 * tags, the explicit tool list, weapon platforms, the under barrel unit and raw elements. Every group is
 * closed and says in one line what it holds. Whether the result is valid is the backend's call: it
 * answers in the plan.
 */
export function CeExtrasEditor({ block, onChange, options, ammoSets }: CeExtrasEditorProps) {
  const tags = block.extraTags ?? [];
  const raw = block.rawExtras ?? [];
  return (
    <div class="flex flex-col gap-2" data-ce-extras>
      <CeOptionsList options={options ?? []} block={block} onChange={onChange} />
      <CeBowFields block={block} onChange={onChange} />
      <Group
        title={t('ceblock.tags.title')}
        summary={tags.length > 0 ? tn('ceblock.tags.count', tags.length) : t('ceblock.tags.none')}
      >
        <StringListField
          label={t('ceblock.tags.label')}
          help={t('ceblock.tags.help')}
          values={tags}
          onChange={(extraTags) => onChange({ extraTags })}
        />
      </Group>
      <CeToolPlanFields block={block} onChange={onChange} />
      <CePlatformFields block={block} onChange={onChange} />
      <CeUnderBarrelFields block={block} onChange={onChange} {...(ammoSets ? { ammoSets } : {})} />
      <Group
        title={t('ceblock.raw.title')}
        summary={raw.length > 0 ? tn('ceblock.raw.count', raw.length) : t('ceblock.raw.none')}
      >
        <RawNodeListField
          label={t('ceblock.raw.label')}
          help={t('ceblock.raw.help')}
          nodes={raw}
          onChange={(rawExtras) => onChange({ rawExtras })}
        />
      </Group>
    </div>
  );
}
