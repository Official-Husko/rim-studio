import type { ApiError } from 'rimstudio-ipc-types';
import { clientErrorId, normalizeError } from '~/shared/ipc';
import { getCurrentProject, openProjectAt, type ProjectRef } from '~/shared/project';

/** Error code of a project whose folder could not be opened again. */
export const PROJECT_MISSING = 'patches.project-missing';

const NOT_OPEN = 'project.not-open';

/**
 * Run a call against the project. When the backend does not know the project id any more (it was
 * restarted), the folder is opened again once and the call is repeated with the new id.
 */
export async function withOpenProject<T>(
  project: ProjectRef,
  run: (projectId: string) => Promise<T>,
): Promise<T> {
  try {
    return await run(getCurrentProject()?.projectId ?? project.projectId);
  } catch (thrown) {
    const error = normalizeError(thrown);
    if (error.code !== NOT_OPEN) throw error;
  }
  try {
    const summary = await openProjectAt(project.path);
    return await run(summary.projectId);
  } catch (thrown) {
    const error = normalizeError(thrown);
    if (
      error.code !== NOT_OPEN &&
      error.code !== 'io.not-found' &&
      error.code !== 'io.invalid-path'
    ) {
      throw error;
    }
    const missing: ApiError = {
      code: PROJECT_MISSING,
      message: error.message,
      errorId: clientErrorId(),
    };
    throw missing;
  }
}
