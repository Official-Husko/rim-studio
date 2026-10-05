import { Banner, FormField, KeyValueList, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { ammoSetLabel, describeResolved, findArchetype } from './wizard-model';
import type { WizardStore } from './wizard-store';
import { SectionHeading } from './SectionHeading';

export interface NameStepProps {
  store: WizardStore;
  /** Called when the user presses Enter in a field. */
  onSubmit: () => void;
}

const KEY_TEXT = {
  action: 'designer.wizard.action.title',
  rof: 'designer.wizard.rof.title',
  calibre: 'designer.wizard.calibre.title',
  handling: 'designer.wizard.handling.title',
  tier: 'designer.wizard.tier.title',
} as const;

/** Step four: the label and the def name of the new weapon, and what is about to be created. */
export function NameStep({ store, onSubmit }: NameStepProps) {
  const catalog = store.catalog.value;
  const proposal = store.proposal.value;
  const choice = store.choice.value;
  const archetype = findArchetype(catalog, choice.archetypeId);
  const error = store.error.value;
  const rows =
    catalog && archetype && proposal
      ? describeResolved(
          catalog,
          archetype,
          proposal.resolved,
          ammoSetLabel(store, proposal.resolved.ammoSet),
        )
      : [];
  const usesCe = choice.ceCalibre && choice.descriptors.ammoSet !== undefined;
  return (
    <form
      class="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      {error ? (
        <Banner tone="error" title={error.code}>
          {error.message}
        </Banner>
      ) : null}
      <div class="grid grid-cols-1 gap-3 lg:grid-cols-2">
        <FormField label={t('designer.field.label')} help={t('designer.wizard.name.labelHelp')}>
          <TextField value={store.label.value} onValueChange={store.setLabel} />
        </FormField>
        <FormField
          label={t('designer.wizard.name.prefix')}
          help={t('designer.wizard.name.prefixHelp')}
        >
          <TextField value={store.prefix.value} onValueChange={store.setPrefix} placeholder="RS_" />
        </FormField>
        <FormField label={t('designer.field.defName')} help={t('designer.help.defName')} required>
          <TextField value={store.defName.value} onValueChange={store.setDefName} />
        </FormField>
      </div>
      <section class="flex flex-col gap-2" aria-label={t('designer.wizard.name.creating')}>
        <SectionHeading>{t('designer.wizard.name.creating')}</SectionHeading>
        <KeyValueList
          items={[
            { key: t('designer.wizard.name.type'), value: archetype?.label ?? '' },
            ...rows.map((r) => ({ key: t(KEY_TEXT[r.key]), value: r.text })),
          ]}
        />
      </section>
      <Banner tone="info" title={t('designer.wizard.name.vanillaTitle')}>
        <div class="flex flex-col gap-1">
          <span>{t('designer.wizard.name.vanilla')}</span>
          {usesCe ? <span>{t('designer.wizard.name.ceFilled')}</span> : null}
        </div>
      </Banner>
    </form>
  );
}
