import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { DraftDto } from 'rimstudio-ipc-types';
import { createEditorStore } from '../../editor-store';
import { createQuizStore } from '../../quiz-store';
import { fixture, installTransport, manualScheduler } from '../../testSupport';
import { QuizStepper } from './QuizStepper';

function setup() {
  const transport = installTransport();
  const editor = createEditorStore({
    projectId: () => 'p-1',
    scheduler: manualScheduler().scheduler,
  });
  editor.open({
    id: 'd-1',
    defName: 'TM_Sword',
    label: 'sword',
    kind: 'melee',
    updatedAtMs: 1,
    draft: fixture<DraftDto>('designer-draft-melee'),
  });
  return { transport, quiz: createQuizStore(editor) };
}

describe('QuizStepper', () => {
  it('shows the first question, the progress and the live estimate', async () => {
    const { quiz } = setup();
    renderWithProviders(<QuizStepper store={quiz} />);
    await quiz.start();
    await waitFor(() => expect(screen.getByText('Question 1 of about 6')).toBeTruthy());
    expect(screen.getByText('Which tier is the weapon?')).toBeTruthy();
    expect(screen.getByText('Live estimate')).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Back' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('answers through the buttons and can skip, say not sure or use what it has', async () => {
    const { quiz, transport } = setup();
    renderWithProviders(<QuizStepper store={quiz} />);
    await quiz.start();
    await waitFor(() => screen.getByRole('button', { name: 'Medieval (5)' }));
    fireEvent.click(screen.getByRole('button', { name: 'Medieval (5)' }));
    await waitFor(() =>
      expect(transport.calls.some((c) => c.name === 'designer_quiz_answer')).toBe(true),
    );
    for (const name of ['Not sure', 'Skip question', 'Use what I have', 'Pause']) {
      expect(screen.getByRole('button', { name })).toBeTruthy();
    }
  });

  it('closes with Pause and keeps nothing on screen', async () => {
    const { quiz } = setup();
    renderWithProviders(<QuizStepper store={quiz} />);
    await quiz.start();
    await waitFor(() => screen.getByRole('button', { name: 'Pause' }));
    fireEvent.click(screen.getByRole('button', { name: 'Pause' }));
    expect(quiz.open.value).toBe(false);
  });

  it('shows the error of a refused answer', async () => {
    const { quiz } = setup();
    renderWithProviders(<QuizStepper store={quiz} />);
    quiz.open.value = true;
    quiz.error.value = {
      code: 'designer.quiz-wrong-answer',
      message: 'not for this question',
      errorId: 'e-1',
    };
    await waitFor(() => expect(screen.getByText('not for this question')).toBeTruthy());
  });
});
