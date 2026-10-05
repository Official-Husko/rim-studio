import { signal } from '@preact/signals';
import type { ApiError, DesignerLintFilesResult } from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import type { ProjectRef } from '~/shared/project';
import { lintProjectFiles } from './api';
import { withOpenProject } from './session';

export type LintState =
  | { phase: 'idle' }
  | { phase: 'running' }
  | { phase: 'ready'; result: DesignerLintFilesResult }
  | { phase: 'error'; error: ApiError };

export const lint = signal<LintState>({ phase: 'idle' });

/** Counts up when the result is forgotten, so a view that shows it runs the lint again. */
export const lintEpoch = signal(0);

let sequence = 0;

/**
 * Lint the patch files of the project, hand written and generated alike. The backend reads the files and
 * runs the rules; the result is only kept for display. Writes nothing.
 */
export async function runLint(project: ProjectRef): Promise<void> {
  const mine = ++sequence;
  lint.value = { phase: 'running' };
  try {
    const result = await withOpenProject(project, (id) => lintProjectFiles(id));
    if (mine === sequence) lint.value = { phase: 'ready', result };
  } catch (thrown) {
    if (mine === sequence) lint.value = { phase: 'error', error: normalizeError(thrown) };
  }
}

/** Forget the lint result (the project changed). */
export function resetLint(): void {
  sequence += 1;
  lint.value = { phase: 'idle' };
  lintEpoch.value += 1;
}
