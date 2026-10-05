import type { DraftDto, PreviewDto } from 'rimstudio-ipc-types';
import { useState } from 'preact/hooks';
import { Banner, Button, Panel, SegmentedControl } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { EstimateSummary } from './EstimateSummary';

export interface EstimatePanelProps {
  draft: DraftDto;
  estimate: PreviewDto['estimate'];
  onChange: (next: DraftDto) => void;
  /** Fill the empty fields from the suggestions; returns how many were filled. */
  onFill: () => number;
  onStartQuiz: () => void;
}

const CHOICES = ['weaker', 'typical', 'stronger'] as const;
type Choice = (typeof CHOICES)[number];

function readChoice(answers: DraftDto['answers']): Choice {
  const raw = answers?.strength as { kind?: string; choice?: string } | undefined;
  return CHOICES.find((c) => c === raw?.choice) ?? 'typical';
}

function answeredCount(answers: DraftDto['answers']): number {
  return Object.entries(answers ?? {}).filter(([key, value]) => {
    if (key === 'strength') return false;
    return !(value as { auto?: boolean } | null)?.auto;
  }).length;
}

/** How strong the weapon should be: a three way choice, or the comparison quiz. */
export function EstimatePanel({
  draft,
  estimate,
  onChange,
  onFill,
  onStartQuiz,
}: EstimatePanelProps) {
  const [filled, setFilled] = useState<number | undefined>(undefined);
  const anchored = draft.calibration === 'anchored';
  const quiz = draft.calibration === 'quiz';
  const choice = readChoice(draft.answers);
  const answered = answeredCount(draft.answers);
  return (
    <Panel title={t('designer.panel.estimate')} collapsible>
      <div class="flex flex-col gap-3">
        {anchored ? (
          <Banner tone="info">
            {t('designer.estimate.anchored', { source: draft.clonedFrom ?? '' })}
          </Banner>
        ) : (
          <SegmentedControl
            label={t('designer.estimate.mode')}
            value={quiz ? 'quiz' : 'simple'}
            onValueChange={(mode) =>
              onChange({ ...draft, calibration: mode === 'quiz' ? 'quiz' : 'simple' })
            }
            options={[
              { value: 'simple', label: t('designer.estimate.simple') },
              { value: 'quiz', label: t('designer.estimate.calibrate') },
            ]}
          />
        )}
        {!anchored && !quiz ? (
          <SegmentedControl
            label={t('designer.estimate.strengthChoice')}
            value={choice}
            onValueChange={(next) =>
              onChange({
                ...draft,
                answers: { ...(draft.answers ?? {}), strength: { kind: 'choice', choice: next } },
              })
            }
            options={CHOICES.map((c) => ({
              value: c,
              label:
                c === 'weaker'
                  ? t('designer.estimate.weaker')
                  : c === 'typical'
                    ? t('designer.estimate.typical')
                    : t('designer.estimate.stronger'),
            }))}
          />
        ) : null}
        {quiz ? (
          <div class="flex items-center gap-3">
            <Button variant="primary" onClick={onStartQuiz}>
              {answered === 0 ? t('designer.quiz.start') : t('designer.quiz.continue')}
            </Button>
            <span class="text-small text-muted">{tn('designer.quiz.answered', answered)}</span>
          </div>
        ) : null}
        {estimate ? <EstimateSummary estimate={estimate} /> : null}
        <div class="flex items-center gap-3">
          <Button onClick={() => setFilled(onFill())}>{t('designer.estimate.fill')}</Button>
          {filled !== undefined ? (
            <span role="status" class="text-small text-muted">
              {filled === 0
                ? t('designer.estimate.filledNone')
                : tn('designer.estimate.filled', filled)}
            </span>
          ) : null}
        </div>
      </div>
    </Panel>
  );
}
