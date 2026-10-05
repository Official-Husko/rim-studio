import type { ArchetypeProposalDto, FitLevelDto } from 'rimstudio-ipc-types';
import { Badge, Banner, ProgressBar } from 'rimstudio-ui';
import { t, type MessageKey } from '~/shared/i18n';
import { sentence } from './wizard-rows';

const LEVEL: Record<FitLevelDto, MessageKey> = {
  typical: 'designer.fit.typical',
  plausible: 'designer.fit.plausible',
  unusual: 'designer.fit.unusual',
};

const TONE = { typical: 'success', plausible: 'info', unusual: 'warning' } as const;

const VERDICT_LINE: Record<FitLevelDto, MessageKey> = {
  typical: 'designer.wizard.verdict.typical',
  plausible: 'designer.wizard.verdict.plausible',
  unusual: 'designer.wizard.verdict.unusual',
};

function levelOf(verdict: string | undefined): FitLevelDto | undefined {
  return verdict === 'typical' || verdict === 'plausible' || verdict === 'unusual'
    ? verdict
    : undefined;
}

export interface FitSummaryProps {
  proposal: ArchetypeProposalDto;
}

/** The verdict of the fit meter, where the strength lands among the class, and the plain notes. */
export function FitSummary({ proposal }: FitSummaryProps) {
  const level = levelOf(proposal.verdict);
  const { strength, fit } = proposal;
  const percent = Math.round(strength.achievedPercentile * 100);
  return (
    <div class="flex flex-col gap-3">
      {level && fit?.typicality !== undefined ? (
        <div class="flex flex-col gap-1">
          <div class="flex items-center justify-between gap-3">
            <span class="text-body text-fg">{t('designer.wizard.verdict.label')}</span>
            <span class="flex items-center gap-2 font-mono text-mono text-fg">
              {t('designer.wizard.verdict.outOf', { value: Math.round(fit.typicality) })}
              <Badge tone={TONE[level]}>{t(LEVEL[level])}</Badge>
            </span>
          </div>
          <ProgressBar
            label={t('designer.wizard.verdict.label')}
            value={fit.typicality / 100}
            tone={level === 'unusual' ? 'warning' : 'accent'}
          />
        </div>
      ) : null}
      {level ? <p class="text-body text-fg">{t(VERDICT_LINE[level])}</p> : null}
      {fit ? (
        <p class="text-small text-muted">
          {t('designer.fit.summary', {
            typical: fit.summary.typical,
            plausible: fit.summary.plausible,
            unusual: fit.summary.unusual,
          })}
        </p>
      ) : null}
      <p class="text-small text-muted">{t('designer.wizard.verdict.strength', { percent })}</p>
      <p class="text-small text-muted">
        {t('designer.wizard.verdict.class', { label: strength.classLabel })}
      </p>
      {fit?.notices.rough ? (
        <Banner tone="warning" title={t('designer.wizard.thin.title')}>
          {t('designer.wizard.thin.body', { n: fit.notices.classN })}
        </Banner>
      ) : null}
      {strength.clamped ? (
        <Banner tone="warning" title={t('designer.wizard.clamped.title')}>
          {t('designer.wizard.clamped.body')}
        </Banner>
      ) : null}
      {proposal.notes.length > 0 ? (
        <Banner tone="info" title={t('designer.wizard.notes.title')}>
          <ul class="flex flex-col gap-1">
            {proposal.notes.map((note) => (
              <li key={note}>{sentence(note)}</li>
            ))}
          </ul>
        </Banner>
      ) : null}
    </div>
  );
}
