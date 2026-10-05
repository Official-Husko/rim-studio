import { describe, expect, it } from 'vitest';
import type { DraftDto } from 'rimstudio-ipc-types';
import { createEditorStore } from './editor-store';
import { createQuizStore } from './quiz-store';
import { fixture, installTransport, manualScheduler, settle } from './testSupport';

function setup(handlers: Parameters<typeof installTransport>[0] = {}) {
  const transport = installTransport(handlers);
  const clock = manualScheduler();
  const editor = createEditorStore({ projectId: () => 'p-1', scheduler: clock.scheduler });
  editor.open({
    id: 'd-1',
    defName: 'TM_Sword',
    label: 'test sword',
    kind: 'melee',
    updatedAtMs: 1,
    draft: fixture<DraftDto>('designer-draft-melee'),
  });
  return { transport, editor, quiz: createQuizStore(editor) };
}

describe('quiz store', () => {
  it('opens at the first question', async () => {
    const { quiz } = setup();
    await quiz.start();
    expect(quiz.open.value).toBe(true);
    expect(quiz.step.value?.prompt?.question.kind).toBe('tier');
  });

  it('records an answer in the open draft and moves on', async () => {
    const { quiz, editor, transport } = setup();
    await quiz.start();
    await quiz.answer({ kind: 'tier', tier: 1 });
    const call = transport.calls.find((c) => c.name === 'designer_quiz_answer')?.request as {
      questionId: string;
    };
    expect(call.questionId).toBe('tier');
    expect(Object.keys(editor.draft.value?.answers ?? {})).toContain('tier');
    expect(editor.saveState.value).toBe('dirty');
    expect(quiz.step.value?.answered).toBe(1);
  });

  it('shows an error without closing the stepper', async () => {
    const { quiz } = setup({
      designer_quiz_answer: () => {
        throw { code: 'designer.quiz-wrong-answer', message: 'no', errorId: 'e-2' };
      },
    });
    await quiz.start();
    await quiz.answer({ kind: 'weaker' });
    expect(quiz.error.value?.code).toBe('designer.quiz-wrong-answer');
    expect(quiz.open.value).toBe(true);
  });

  it('takes the last answer back and closes with the answers kept', async () => {
    const { quiz, transport } = setup();
    await quiz.start();
    await quiz.back();
    await settle();
    expect(transport.calls.some((c) => c.name === 'designer_quiz_back')).toBe(true);
    quiz.close();
    expect(quiz.open.value).toBe(false);
    expect(quiz.step.value).toBeUndefined();
  });
});
