import type { CookOffKindDto, CustomAmmoItemDto, SourcedDto } from 'rimstudio-ipc-types';
import { FormField, SegmentedControl } from 'rimstudio-ui';
import { Group } from '~/shared/ce/Group';
import { RawNodeListField } from '~/shared/ce/RawNodeListField';
import { StringListField } from '~/shared/ce/StringListField';
import { t } from '~/shared/i18n';
import { AmmoNumberField, AmmoTextField } from './fields';
import { diagnosticsUnder, typedNumber, typePointer } from './model';
import { PairListField, type PairRow } from './PairListField';
import type { CustomAmmoStore } from './store';

export interface ItemFieldsProps {
  store: CustomAmmoStore;
  index: number;
}

const AUTO = 'auto';

/** The ammo item of one type: its weight, its stack, its categories, what it does when it burns and its own stats. */
export function ItemFields({ store, index }: ItemFieldsProps) {
  const item: CustomAmmoItemDto = store.custom.value.types[index]?.item ?? {};
  const field = { store, index } as const;
  const stats: PairRow[] = Object.entries(item.statBases ?? {}).map(([name, v]) => ({
    name,
    amount: v.value,
  }));
  return (
    <div class="flex flex-col gap-3">
      <Group title={t('ammo.item.basics')} open>
        <div class="grid grid-cols-[repeat(auto-fit,minmax(14rem,1fr))] gap-x-4 gap-y-3">
          <AmmoTextField
            {...field}
            pointer="/label"
            label={t('ammo.field.item-label')}
            help={t('ammo.field.item-label-help')}
          />
          <AmmoTextField
            {...field}
            pointer="/item/parent"
            label={t('ammo.field.parent')}
            help={t('ammo.field.item-parent-help')}
          />
          <AmmoNumberField
            {...field}
            pointer="/item/mass"
            label={t('ammo.field.mass')}
            unit={t('ammo.unit.mass')}
            step={0.001}
          />
          <AmmoNumberField
            {...field}
            pointer="/item/bulk"
            label={t('ammo.field.bulk')}
            step={0.001}
          />
          <AmmoNumberField
            {...field}
            pointer="/item/marketValue"
            label={t('ammo.field.market')}
            help={t('ammo.field.market-help')}
            unit={t('ammo.unit.silver')}
            step={0.01}
          />
          <AmmoNumberField
            {...field}
            pointer="/item/stackLimit"
            label={t('ammo.field.stack')}
            whole
            step={10}
          />
          <AmmoTextField
            {...field}
            pointer="/item/techLevel"
            label={t('ammo.field.tech')}
            help={t('ammo.field.tech-help')}
          />
        </div>
        <AmmoTextField
          {...field}
          pointer="/description"
          label={t('ammo.field.item-description')}
          multiline
        />
        <FormField label={t('ammo.field.cook-off')} help={t('ammo.field.cook-off-help')}>
          <SegmentedControl
            label={t('ammo.field.cook-off')}
            value={item.cookOff ?? AUTO}
            onValueChange={(v) =>
              store.setField(index, '/item/cookOff', v === AUTO ? undefined : (v as CookOffKindDto))
            }
            options={[
              { value: AUTO, label: t('ammo.cook.auto') },
              { value: 'projectile', label: t('ammo.cook.projectile') },
              { value: 'detonate', label: t('ammo.cook.detonate') },
              { value: 'none', label: t('ammo.cook.none') },
            ]}
          />
        </FormField>
      </Group>
      <Group title={t('ammo.item.lists')} summary={t('ammo.item.lists-help')}>
        <StringListField
          label={t('ammo.field.categories')}
          help={t('ammo.field.categories-help')}
          values={item.thingCategories ?? []}
          onChange={(v) => store.setField(index, '/item/thingCategories', v)}
        />
        <StringListField
          label={t('ammo.field.trade-tags')}
          help={t('ammo.field.trade-tags-help')}
          values={item.tradeTags ?? []}
          onChange={(v) => store.setField(index, '/item/tradeTags', v)}
        />
        <PairListField
          label={t('ammo.field.stats')}
          nameLabel={t('ammo.field.stat-name')}
          amountLabel={t('ammo.field.stat-value')}
          addLabel={t('ammo.field.stat-add')}
          help={t('ammo.field.stats-help')}
          rows={stats}
          pointer={typePointer(index, '/item/statBases')}
          diagnostics={diagnosticsUnder(
            store.diagnostics.value,
            typePointer(index, '/item/statBases'),
          )}
          onChange={(rows) => {
            const map: Record<string, SourcedDto<number>> = {};
            for (const r of rows) map[r.name] = typedNumber(r.amount ?? 0);
            store.setField(index, '/item/statBases', Object.keys(map).length > 0 ? map : undefined);
          }}
        />
      </Group>
      <Group title={t('ammo.raw.title')} summary={t('ammo.raw.summary')}>
        <RawNodeListField
          label={t('ammo.raw.item')}
          help={t('ammo.raw.help')}
          nodes={item.extra ?? []}
          onChange={(nodes) => store.setField(index, '/item/extra', nodes)}
        />
      </Group>
    </div>
  );
}
