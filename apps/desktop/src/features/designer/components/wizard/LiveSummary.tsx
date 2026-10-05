import { Badge, KeyValueList, Spinner } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t, type MessageKey } from '~/shared/i18n';
import { statLabel } from '../../model/labels';
import { ammoSetLabel, describeResolved, findArchetype } from './wizard-model';
import { headlineStats } from './wizard-rows';
import type { WizardStore } from './wizard-store';

const KEY_TEXT: Record<'action' | 'rof' | 'calibre' | 'handling' | 'tier', MessageKey> = {
  action: 'designer.wizard.action.title',
  rof: 'designer.wizard.rof.title',
  calibre: 'designer.wizard.calibre.title',
  handling: 'designer.wizard.handling.title',
  tier: 'designer.wizard.tier.title',
};

const TONE = { typical: 'success', plausible: 'info', unusual: 'warning' } as const;

const VERDICT_TEXT: Record<keyof typeof TONE, MessageKey> = {
  typical: 'designer.fit.typical',
  plausible: 'designer.fit.plausible',
  unusual: 'designer.fit.unusual',
};

export interface LiveSummaryProps {
  store: WizardStore;
}

/** The side summary: the type, what was chosen, the headline numbers and the verdict, live. */
export function LiveSummary({ store }: LiveSummaryProps) {
  const catalog = store.catalog.value;
  const proposal = store.proposal.value;
  const archetype = findArchetype(catalog, store.choice.value.archetypeId);
  if (!catalog || !archetype) {
    return <p class="text-small text-muted">{t('designer.wizard.summary.empty')}</p>;
  }
  const raw = proposal?.verdict;
  const verdict = raw === 'typical' || raw === 'plausible' || raw === 'unusual' ? raw : undefined;
  const chosen = proposal
    ? describeResolved(
        catalog,
        archetype,
        proposal.resolved,
        ammoSetLabel(store, proposal.resolved.ammoSet),
      )
    : [];
  return (
    <div class="flex flex-col gap-3" aria-busy={store.proposing.value ? 'true' : 'false'}>
      <div class="flex items-center justify-between gap-2">
        <h3 class="text-body font-semibold text-fg">{archetype.label}</h3>
        {store.proposing.value ? <Spinner label={t('designer.wizard.proposing')} /> : null}
      </div>
      {verdict ? (
        <div>
          <Badge tone={TONE[verdict]}>{t(VERDICT_TEXT[verdict])}</Badge>
        </div>
      ) : null}
      {chosen.length > 0 ? (
        <KeyValueList
          label={t('designer.wizard.summary.chosen')}
          items={chosen.map((c) => ({ key: t(KEY_TEXT[c.key]), value: c.text }))}
        />
      ) : null}
      {proposal ? (
        <KeyValueList
          label={t('designer.wizard.summary.numbers')}
          items={headlineStats(proposal).map((s) => ({
            key: statLabel(s.stat),
            value: formatNumber(s.value, 2),
            mono: true,
          }))}
        />
      ) : null}
    </div>
  );
}
