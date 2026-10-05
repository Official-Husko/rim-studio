import { Banner, Spinner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { BalanceControl } from './BalanceControl';
import { CompareTable } from './CompareTable';
import { FitSummary } from './FitSummary';
import { ProposalGroup } from './ProposalGroup';
import { SectionHeading } from './SectionHeading';
import { groupRows } from './wizard-rows';
import type { WizardStore } from './wizard-store';

export interface ResultStepProps {
  store: WizardStore;
}

/** Step three: the proposed numbers with their reasons, the verdict and the comparison. */
export function ResultStep({ store }: ResultStepProps) {
  const catalog = store.catalog.value;
  const proposal = store.proposal.value;
  const error = store.proposeError.value;
  const choice = store.choice.value;
  if (error) {
    return (
      <Banner tone="error" title={error.code}>
        {error.message}
      </Banner>
    );
  }
  if (!proposal || !catalog) {
    return (
      <div class="flex items-center gap-2 text-muted">
        <Spinner label={t('designer.wizard.proposing')} />
        <span>{t('designer.wizard.proposing')}</span>
      </div>
    );
  }
  const melee = proposal.kind === 'melee';
  const groups = groupRows(proposal, store.previous.value, store.target.value);
  return (
    <div class="flex flex-col gap-4" aria-busy={store.proposing.value ? 'true' : 'false'}>
      <section class="flex flex-col gap-2" aria-label={t('designer.wizard.balance.title')}>
        <SectionHeading>{t('designer.wizard.balance.title')}</SectionHeading>
        <BalanceControl catalog={catalog} value={choice.balance} onChange={store.setBalance} />
      </section>
      <FitSummary proposal={proposal} />
      <div class="grid grid-cols-1 gap-3 xl:grid-cols-2">
        {groups.map((group) => (
          <ProposalGroup
            key={group.id}
            group={group}
            melee={melee}
            extras={group.id === 'weight' ? proposal : undefined}
          />
        ))}
      </div>
      {proposal.fit ? <CompareTable report={proposal.fit} /> : null}
    </div>
  );
}
