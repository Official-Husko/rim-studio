import type { ConvertCandidateDto } from 'rimstudio-ipc-types';
import { Banner, SegmentedControl } from 'rimstudio-ui';
import { useState } from 'preact/hooks';
import { t, tn } from '~/shared/i18n';
import {
  effectiveAnswer,
  familyAnswers,
  ownAnswers,
  setAnswer,
  type AnswerScope,
} from './answerStore';
import { AskRow } from './AskRow';
import { familyOf } from './model';
import { rankNote, rankedOptions } from './rank';
import { suggestions } from './suggestStore';

export interface QuestionsFormProps {
  candidate: ConvertCandidateDto;
  /** How many weapons of the scan share the family key of this one (itself included). */
  familySize: number;
}

/** The open questions of one weapon, answered for the weapon or for its whole family. */
export function QuestionsForm({ candidate, familySize }: QuestionsFormProps) {
  const [scope, setScope] = useState<AnswerScope>('weapon');
  const own = ownAnswers.value;
  const family = familyAnswers.value;
  const choices = suggestions.value[familyOf(candidate) || candidate.defName];
  const shareable = familyOf(candidate) !== '' && familySize > 1;
  const activeScope: AnswerScope = shareable ? scope : 'weapon';
  const answered = candidate.asks.filter(
    (ask) => effectiveAnswer(candidate, ask, own, family).value !== undefined,
  ).length;

  if (candidate.asks.length === 0) {
    return <Banner tone="success">{t('patches.questions.none')}</Banner>;
  }
  return (
    <div class="flex flex-col gap-4">
      <div class="flex flex-wrap items-center gap-3">
        <p class="m-0 flex-1 text-small text-muted">
          {t('patches.questions.progress', { done: answered, total: candidate.asks.length })}
        </p>
        {shareable ? (
          <div class="shrink-0">
            <SegmentedControl
              label={t('patches.scope.label')}
              value={scope}
              onValueChange={(v) => setScope(v === 'family' ? 'family' : 'weapon')}
              options={[
                { value: 'weapon', label: t('patches.scope.weapon') },
                { value: 'family', label: tn('patches.scope.family', familySize) },
              ]}
            />
          </div>
        ) : null}
      </div>
      {shareable && activeScope === 'family' ? (
        <Banner tone="info">
          {t('patches.scope.family-help', { family: familyOf(candidate) })}
        </Banner>
      ) : null}
      {candidate.asks.map((ask) => {
        const { value, from } = effectiveAnswer(candidate, ask, own, family);
        const choice = choices?.[ask.field];
        const note = rankNote(choice);
        return (
          <AskRow
            key={ask.field}
            ask={ask}
            value={value}
            from={from === 'family' && activeScope === 'family' ? undefined : from}
            options={ask.kind === 'choice' ? rankedOptions(ask, choice) : []}
            {...(note ? { rankNote: note } : {})}
            onChange={(v) => setAnswer(candidate, activeScope, ask, v)}
          />
        );
      })}
    </div>
  );
}
