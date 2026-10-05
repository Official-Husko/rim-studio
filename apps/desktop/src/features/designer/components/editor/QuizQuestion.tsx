import type { QuestionDto, QuizAnswerDto } from 'rimstudio-ipc-types';
import { useState } from 'preact/hooks';
import { Button, NumberField, FormField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { statLabel } from '../../model/labels';
import { AnchorCardView } from './AnchorCardView';

export interface QuizQuestionProps {
  question: QuestionDto;
  disabled: boolean;
  onAnswer: (answer: QuizAnswerDto) => void;
}

function TypedAnswer({
  stat,
  disabled,
  onAnswer,
}: {
  stat: string;
  disabled: boolean;
  onAnswer: QuizQuestionProps['onAnswer'];
}) {
  const [value, setValue] = useState<number | undefined>(undefined);
  return (
    <div class="flex items-end gap-2">
      <div class="w-48">
        <FormField label={t('designer.quiz.typedLabel', { stat: statLabel(stat) })}>
          <NumberField value={value} onValueChange={setValue} step={0.1} />
        </FormField>
      </div>
      <Button
        disabled={disabled || value === undefined}
        onClick={() => value !== undefined && onAnswer({ kind: 'typed', value })}
      >
        {t('designer.quiz.useValue')}
      </Button>
    </div>
  );
}

function ChoiceButtons({
  options,
  disabled,
  onPick,
}: {
  options: Array<{ key: string; text: string }>;
  disabled: boolean;
  onPick: (key: string) => void;
}) {
  return (
    <div class="flex flex-wrap gap-2">
      {options.map((o) => (
        <Button key={o.key} disabled={disabled} onClick={() => onPick(o.key)}>
          {o.text}
        </Button>
      ))}
    </div>
  );
}

/** The body of one question: the anchor cards and the answer buttons that fit its kind. */
export function QuizQuestion({ question, disabled, onAnswer }: QuizQuestionProps) {
  switch (question.kind) {
    case 'tier':
      return (
        <div class="flex flex-col gap-3">
          <p class="text-body">{t('designer.quiz.tier')}</p>
          <ChoiceButtons
            disabled={disabled}
            options={question.options.map((o) => ({
              key: String(o.tier),
              text: `${o.label} (${o.count})`,
            }))}
            onPick={(key) => onAnswer({ kind: 'tier', tier: Number(key) })}
          />
        </div>
      );
    case 'role':
      return (
        <div class="flex flex-col gap-3">
          <p class="text-body">{t('designer.quiz.role')}</p>
          <ChoiceButtons
            disabled={disabled}
            options={question.options.map((o) => ({ key: o.name, text: `${o.name} (${o.count})` }))}
            onPick={(key) => onAnswer({ kind: 'role', role: key })}
          />
        </div>
      );
    case 'group':
      return (
        <div class="flex flex-col gap-3">
          <p class="text-body">{t('designer.quiz.group')}</p>
          <ChoiceButtons
            disabled={disabled}
            options={question.options.map((o) => ({ key: o.name, text: `${o.name} (${o.count})` }))}
            onPick={(key) => onAnswer({ kind: 'group', group: key })}
          />
        </div>
      );
    case 'compare':
      return (
        <div class="flex flex-col gap-3">
          <p class="text-body">{t('designer.quiz.compare', { label: question.anchor.label })}</p>
          <AnchorCardView card={question.anchor} />
          <ChoiceButtons
            disabled={disabled}
            options={[
              { key: 'weaker', text: t('designer.quiz.weaker') },
              { key: 'same', text: t('designer.quiz.same') },
              { key: 'stronger', text: t('designer.quiz.stronger') },
            ]}
            onPick={(key) => onAnswer({ kind: key as 'weaker' | 'same' | 'stronger' })}
          />
        </div>
      );
    case 'closer-to':
      return (
        <div class="flex flex-col gap-3">
          <p class="text-body">{t('designer.quiz.closer')}</p>
          <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
            <AnchorCardView card={question.lower} caption={t('designer.quiz.lower')} />
            <AnchorCardView card={question.upper} caption={t('designer.quiz.upper')} />
          </div>
          <ChoiceButtons
            disabled={disabled}
            options={[
              { key: 'closer-to-lower', text: t('designer.quiz.closerLower') },
              { key: 'closer-to-upper', text: t('designer.quiz.closerUpper') },
            ]}
            onPick={(key) => onAnswer({ kind: key as 'closer-to-lower' | 'closer-to-upper' })}
          />
        </div>
      );
    case 'interval':
      return (
        <div class="flex flex-col gap-3">
          <p class="text-body">{t('designer.quiz.interval', { stat: statLabel(question.stat) })}</p>
          <ChoiceButtons
            disabled={disabled}
            options={question.bins.map((b, i) => ({ key: String(i), text: b.label }))}
            onPick={(key) => onAnswer({ kind: 'bin', index: Number(key) })}
          />
          <TypedAnswer stat={question.stat} disabled={disabled} onAnswer={onAnswer} />
        </div>
      );
    case 'vs-anchor':
      return (
        <div class="flex flex-col gap-3">
          <p class="text-body">
            {t('designer.quiz.vsAnchor', {
              stat: statLabel(question.stat),
              label: question.anchor.label,
            })}
          </p>
          <AnchorCardView card={question.anchor} />
          <ChoiceButtons
            disabled={disabled}
            options={[
              { key: 'lower', text: t('designer.quiz.lowerThan') },
              { key: 'similar', text: t('designer.quiz.similar') },
              { key: 'higher', text: t('designer.quiz.higherThan') },
            ]}
            onPick={(key) =>
              onAnswer({ kind: 'bucket', bucket: key as 'lower' | 'similar' | 'higher' })
            }
          />
          <TypedAnswer stat={question.stat} disabled={disabled} onAnswer={onAnswer} />
        </div>
      );
  }
}
