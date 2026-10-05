import { currentProject as shared, openProjectAt, setCurrentProject } from '~/shared/project';

/** The project the designer works in. */
export interface ProjectRef {
  projectId: string;
  path: string;
  name: string;
}

/**
 * The designer follows the project of the top bar selector (shared/project), so the Project,
 * Weapons and Patches pages always work in the same folder.
 */

/** The current project, or undefined when none is open. Reading it inside a component subscribes. */
export function currentProject(): ProjectRef | undefined {
  return shared.value;
}

/** Open a folder as the current project. */
export async function selectProject(path: string): Promise<ProjectRef> {
  return openProjectAt(path);
}

/** Forget the project (tests). */
export function clearProject(): void {
  setCurrentProject(undefined);
}

/** Set the project directly (tests). */
export function setProject(ref: ProjectRef | undefined): void {
  setCurrentProject(ref);
}
