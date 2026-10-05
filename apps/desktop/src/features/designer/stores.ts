import type { DraftEntryDto } from 'rimstudio-ipc-types';
import { createAssetStore } from './asset-store';
import { createDraftsStore } from './drafts-store';
import { createEditorStore } from './editor-store';
import { createOutputStore } from './output-store';
import { currentProject } from './project-source';
import { createQuizStore } from './quiz-store';
import { createReferenceStore } from './reference-store';
import type { Scheduler } from './scheduler';

/** Every store of the designer, wired together. */
export function createDesigner(
  projectId: () => string | undefined = () => currentProject()?.projectId,
  scheduler?: Scheduler,
) {
  const drafts = createDraftsStore({ projectId });
  const editor = createEditorStore({
    projectId,
    ...(scheduler ? { scheduler } : {}),
    onSaved: drafts.upsert,
  });
  const reference = createReferenceStore();
  const quiz = createQuizStore(editor);
  const assets = createAssetStore({ projectId });
  const output = createOutputStore({
    editor,
    projectId,
    ...(scheduler ? { scheduler } : {}),
  });

  /** Make a draft the open one; the previous one is saved first. */
  async function select(entry: DraftEntryDto): Promise<void> {
    await editor.flush();
    quiz.close();
    drafts.selectedId.value = entry.id;
    editor.open(entry);
  }

  /** Delete a draft; closes it first when it is the open one. */
  async function remove(id: string): Promise<void> {
    if (editor.entryId.peek() === id) editor.close();
    await drafts.remove(id);
  }

  /** Forget the open draft and the list (the project changed). */
  function reset(): void {
    editor.close();
    quiz.close();
    drafts.reset();
  }

  return { drafts, editor, reference, quiz, output, assets, select, remove, reset };
}

export type Designer = ReturnType<typeof createDesigner>;

/** The designer of the running app. */
export const designer: Designer = createDesigner();
