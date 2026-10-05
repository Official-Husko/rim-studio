import { batch, signal } from '@preact/signals';
import type { ApiError, QuizAnswerDto, QuizStepDto } from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import * as api from './api';
import type { EditorStore } from './editor-store';

/** The estimate dialogue of the open draft: one question at a time. */
export function createQuizStore(editor: EditorStore) {
  const open = signal(false);
  const step = signal<QuizStepDto | undefined>(undefined);
  const busy = signal(false);
  const error = signal<ApiError | undefined>(undefined);

  async function run(
    call: () => Promise<{ step: QuizStepDto; draft?: Parameters<EditorStore['edit']>[0] }>,
  ): Promise<void> {
    busy.value = true;
    try {
      const result = await call();
      batch(() => {
        if (result.draft) editor.edit(result.draft);
        step.value = result.step;
        error.value = undefined;
      });
    } catch (thrown) {
      error.value = normalizeError(thrown);
    } finally {
      busy.value = false;
    }
  }

  /** Open the stepper at the next open question of the draft. */
  async function start(): Promise<void> {
    const draft = editor.draft.peek();
    if (!draft) return;
    open.value = true;
    await run(async () => ({ step: await api.quizNext(draft) }));
  }

  /** Answer the question on screen. */
  async function answer(value: QuizAnswerDto): Promise<void> {
    const draft = editor.draft.peek();
    const prompt = step.peek()?.prompt;
    if (!draft || !prompt) return;
    await run(() => api.quizAnswer(draft, prompt.id, value));
  }

  /** Take back the last answer. */
  async function back(): Promise<void> {
    const draft = editor.draft.peek();
    if (!draft) return;
    await run(() => api.quizBack(draft));
  }

  /** Close the stepper; the answers stay in the draft. */
  function close(): void {
    open.value = false;
    step.value = undefined;
    error.value = undefined;
  }

  return { open, step, busy, error, start, answer, back, close };
}

export type QuizStore = ReturnType<typeof createQuizStore>;
