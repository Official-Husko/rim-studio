import type { ArchetypeProposalDto } from 'rimstudio-ipc-types';
import { Chip, Panel } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';
import type { ResultGroup } from './wizard-rows';
import { groupTitle, reasonText, sentence } from './wizard-rows';

export interface ProposalGroupProps {
  group: ResultGroup;
  melee: boolean;
  /** The cost list, stuff and market value, shown with the weight group. */
  extras?: ArchetypeProposalDto | undefined;
}

function CostLines({ proposal }: { proposal: ArchetypeProposalDto }) {
  return (
    <>
      {proposal.costList.map((cost) => (
        <li key={cost.defName} class="flex flex-col gap-0.5 py-2">
          <div class="flex items-baseline justify-between gap-3">
            <span class="text-body text-fg">{cost.defName}</span>
            <span class="flex items-center gap-2">
              <span class="font-mono text-mono text-fg">{`x ${formatNumber(cost.count, 0)}`}</span>
              <Chip kind="derived" />
            </span>
          </div>
          <p class="text-small text-muted">{sentence(cost.reason)}</p>
        </li>
      ))}
      {proposal.stuff ? (
        <li class="flex flex-col gap-0.5 py-2">
          <div class="flex items-baseline justify-between gap-3">
            <span class="text-body text-fg">
              {t('designer.wizard.result.stuff', {
                categories: proposal.stuff.categories.join(', '),
              })}
            </span>
            <span class="flex items-center gap-2">
              <span class="font-mono text-mono text-fg">{`x ${formatNumber(proposal.stuff.count, 0)}`}</span>
              <Chip kind="derived" />
            </span>
          </div>
          <p class="text-small text-muted">{sentence(proposal.stuff.reason)}</p>
        </li>
      ) : null}
      {proposal.marketValue !== undefined ? (
        <li class="flex flex-col gap-0.5 py-2">
          <div class="flex items-baseline justify-between gap-3">
            <span class="text-body text-fg">{t('designer.wizard.result.market')}</span>
            <span class="font-mono text-mono text-fg">{formatNumber(proposal.marketValue, 0)}</span>
          </div>
          <p class="text-small text-muted">{t('designer.wizard.result.marketNote')}</p>
        </li>
      ) : null}
    </>
  );
}

/** A group of proposed numbers: each with its value, the derived chip and the plain reason. */
export function ProposalGroup({ group, melee, extras }: ProposalGroupProps) {
  const title = groupTitle(group.id, melee);
  return (
    <Panel title={title}>
      <ul class="flex flex-col divide-y divide-line-subtle" aria-label={title}>
        {group.rows.map((row) => (
          <li key={row.key} class="flex flex-col gap-0.5 py-2" data-field={row.value.field}>
            <div class="flex items-baseline justify-between gap-3">
              <span class="text-body text-fg">{row.label}</span>
              <span class="flex items-center gap-2">
                {row.was !== undefined ? (
                  <span class="font-mono text-mono-small text-faint">
                    {t('designer.wizard.result.was', { value: formatNumber(row.was, 3) })}
                  </span>
                ) : null}
                <span class="font-mono text-mono text-fg">
                  {formatNumber(row.yours ?? row.value.value, 3)}
                  {row.unit ? <span class="text-faint">{` ${row.unit}`}</span> : null}
                </span>
                <Chip kind="derived" />
              </span>
            </div>
            <p class="text-small text-muted">
              {row.value.locked
                ? t('designer.wizard.result.locked', {
                    proposed: formatNumber(row.value.value, 3),
                  })
                : row.value.write
                  ? reasonText(row.value.reason)
                  : `${reasonText(row.value.reason)} ${t('designer.wizard.result.notWritten')}`}
            </p>
          </li>
        ))}
        {extras ? <CostLines proposal={extras} /> : null}
      </ul>
    </Panel>
  );
}
