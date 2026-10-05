import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { DesignerQuizAnswerResponse, QuestionDto, QuizStepDto } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import { QuizQuestion } from './QuizQuestion';

const card = fixture<QuizStepDto>('designer_quiz_next').estimate?.anchors[0];
if (!card) throw new Error('fixture without anchors');

describe('QuizQuestion', () => {
  it('answers the tier question with the tier of the button', () => {
    const question = fixture<QuizStepDto>('designer_quiz_next').prompt?.question as QuestionDto;
    const onAnswer = vi.fn();
    renderWithProviders(<QuizQuestion question={question} disabled={false} onAnswer={onAnswer} />);
    fireEvent.click(screen.getByRole('button', { name: 'Medieval (5)' }));
    expect(onAnswer).toHaveBeenCalledWith({ kind: 'tier', tier: 1 });
  });

  it('answers the role question with the role name', () => {
    const next = fixture<DesignerQuizAnswerResponse>('designer_quiz_answer');
    const question = next.step.prompt?.question as QuestionDto;
    const onAnswer = vi.fn();
    renderWithProviders(<QuizQuestion question={question} disabled={false} onAnswer={onAnswer} />);
    fireEvent.click(screen.getByRole('button', { name: 'blade (9)' }));
    expect(onAnswer).toHaveBeenCalledWith({ kind: 'role', role: 'blade' });
  });

  it('answers a comparison with weaker, about the same or stronger', () => {
    const onAnswer = vi.fn();
    renderWithProviders(
      <QuizQuestion
        question={{
          kind: 'compare',
          anchor: card,
          remaining: 3,
          asked: 0,
          budget: 5,
          information: 1,
        }}
        disabled={false}
        onAnswer={onAnswer}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'About the same' }));
    expect(onAnswer).toHaveBeenCalledWith({ kind: 'same' });
    expect(screen.getByRole('heading', { name: 'longsword' })).toBeTruthy();
  });

  it('answers a closer-to question for either weapon', () => {
    const onAnswer = vi.fn();
    renderWithProviders(
      <QuizQuestion
        question={{ kind: 'closer-to', lower: card, upper: card }}
        disabled={false}
        onAnswer={onAnswer}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Closer to the second' }));
    expect(onAnswer).toHaveBeenCalledWith({ kind: 'closer-to-upper' });
  });

  it('takes a typed number for an interval question', () => {
    const onAnswer = vi.fn();
    renderWithProviders(
      <QuizQuestion
        question={{
          kind: 'interval',
          stat: 'swing_damage',
          bins: [{ lo: 0, hi: 10, label: 'under 10' }],
          spread: 1,
        }}
        disabled={false}
        onAnswer={onAnswer}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'under 10' }));
    expect(onAnswer).toHaveBeenCalledWith({ kind: 'bin', index: 0 });
    fireEvent.input(screen.getByRole('spinbutton'), { target: { value: '21' } });
    fireEvent.click(screen.getByRole('button', { name: 'Use this value' }));
    expect(onAnswer).toHaveBeenLastCalledWith({ kind: 'typed', value: 21 });
  });

  it('answers a stat comparison with a bucket', () => {
    const onAnswer = vi.fn();
    renderWithProviders(
      <QuizQuestion
        question={{ kind: 'vs-anchor', stat: 'mass', anchor: card, value: 2 }}
        disabled={false}
        onAnswer={onAnswer}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Higher' }));
    expect(onAnswer).toHaveBeenCalledWith({ kind: 'bucket', bucket: 'higher' });
  });

  it('disables the answers while a call runs', () => {
    const question = fixture<QuizStepDto>('designer_quiz_next').prompt?.question as QuestionDto;
    renderWithProviders(<QuizQuestion question={question} disabled onAnswer={() => undefined} />);
    expect(
      (screen.getByRole('button', { name: 'Medieval (5)' }) as HTMLButtonElement).disabled,
    ).toBe(true);
  });
});
