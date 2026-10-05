import { SegmentedControl, Select } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { BalanceControl } from './BalanceControl';
import { CalibreControl } from './CalibreControl';
import { DescriptorBlock } from './DescriptorBlock';
import { RateOfFireControl } from './RateOfFireControl';
import type { WizardStore } from './wizard-store';
import { applies, findArchetype, tierCount } from './wizard-model';
import { Banner } from 'rimstudio-ui';

export interface DescribeStepProps {
  store: WizardStore;
}

/** Step two: only the descriptors that apply to the chosen type, each with what it does. */
export function DescribeStep({ store }: DescribeStepProps) {
  const catalog = store.catalog.value;
  const choice = store.choice.value;
  const archetype = findArchetype(catalog, choice.archetypeId);
  if (!catalog || !archetype)
    return <Banner tone="info">{t('designer.wizard.describe.pick')}</Banner>;
  const d = choice.descriptors;
  const action = catalog.actions.find((a) => a.id === d.action);
  const defaultTier = catalog.tiers.find((x) => x.tier === archetype.defaultTier);
  return (
    <div class="flex flex-col gap-5">
      <p class="text-body text-muted">
        {t('designer.wizard.describe.intro', { name: archetype.label })}
      </p>
      <div class="grid grid-cols-1 gap-5 lg:grid-cols-2">
        {applies(archetype, 'action') ? (
          <DescriptorBlock title={t('designer.wizard.action.title')} help={action?.summary}>
            <SegmentedControl
              label={t('designer.wizard.action.title')}
              options={archetype.actions.flatMap((id) => {
                const found = catalog.actions.find((a) => a.id === id);
                return found ? [{ value: id, label: found.label }] : [];
              })}
              value={d.action ?? ''}
              onValueChange={(id) => store.setDescriptors({ action: id })}
            />
          </DescriptorBlock>
        ) : null}
        {applies(archetype, 'rof') ? (
          <DescriptorBlock
            title={
              archetype.kind === 'melee'
                ? t('designer.wizard.rof.swingTitle')
                : t('designer.wizard.rof.title')
            }
          >
            <RateOfFireControl
              archetype={archetype}
              catalog={catalog}
              value={d.rof}
              onChange={(rof) => store.setDescriptors({ rof })}
            />
          </DescriptorBlock>
        ) : null}
        {applies(archetype, 'calibre') ? (
          <DescriptorBlock title={t('designer.wizard.calibre.title')}>
            <CalibreControl
              archetype={archetype}
              catalog={catalog}
              calibre={d.calibre}
              ammoSet={d.ammoSet}
              ceCalibre={choice.ceCalibre}
              ceCalibres={store.calibres.value}
              onCalibre={(id) => store.setDescriptors({ calibre: id })}
              onAmmoSet={(set) => store.setDescriptors({ ammoSet: set })}
              onUseCe={store.setCeCalibre}
            />
          </DescriptorBlock>
        ) : null}
        {applies(archetype, 'handling') ? (
          <DescriptorBlock
            title={t('designer.wizard.handling.title')}
            help={t('designer.wizard.handling.help')}
          >
            <SegmentedControl
              label={t('designer.wizard.handling.title')}
              options={archetype.handlings.flatMap((id) => {
                const found = catalog.handlings.find((h) => h.id === id);
                return found ? [{ value: id, label: found.label }] : [];
              })}
              value={d.handling ?? ''}
              onValueChange={(id) => store.setDescriptors({ handling: id })}
            />
          </DescriptorBlock>
        ) : null}
        {applies(archetype, 'tier') ? (
          <DescriptorBlock
            title={t('designer.wizard.tier.title')}
            help={t('designer.wizard.tier.help')}
          >
            <Select
              aria-label={t('designer.wizard.tier.title')}
              value={d.tier ?? ''}
              onValueChange={(value) => {
                const next = catalog.tiers.find((x) => x.tier === value);
                store.setDescriptors({ tier: next?.tier });
              }}
              options={[
                {
                  value: '',
                  label: t('designer.wizard.tier.default', {
                    tier: defaultTier?.label ?? archetype.defaultTier,
                  }),
                },
                ...catalog.tiers.map((x) => ({
                  value: x.tier,
                  label: `${x.label} (${tn('designer.wizard.tier.count', tierCount(catalog, x.tier, archetype.kind))})`,
                })),
              ]}
            />
          </DescriptorBlock>
        ) : null}
        {applies(archetype, 'balance') ? (
          <DescriptorBlock title={t('designer.wizard.balance.title')}>
            <BalanceControl catalog={catalog} value={choice.balance} onChange={store.setBalance} />
          </DescriptorBlock>
        ) : null}
      </div>
    </div>
  );
}
