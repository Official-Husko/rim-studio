import type {
  ProjectCreateRequest,
  ProjectFileDto,
  ProjectLayoutCheckDto,
  ProjectScaffoldMissingDto,
  ProjectSummaryDto,
  ProjectTreeDto,
  SourceDto,
} from 'rimstudio-ipc-types';
import { callCommand } from '~/shared/ipc';

// One function per command; no state, no rules. The store calls these.

/** Register a mod folder as a project and read its summary. */
export function openProject(path: string): Promise<ProjectSummaryDto> {
  return callCommand('project_open', { path });
}

/** Write the scaffold of a new mod and open it. */
export function createProject(request: ProjectCreateRequest): Promise<ProjectSummaryDto> {
  return callCommand('project_create', request);
}

/** The annotated tree of a project. */
export function readTree(projectId: string): Promise<ProjectTreeDto> {
  return callCommand('project_tree', { projectId });
}

/** The layout issues of a project. */
export function checkLayout(projectId: string): Promise<ProjectLayoutCheckDto> {
  return callCommand('project_layout_check', { projectId });
}

/** Create the missing standard folders (or list them with dryRun). */
export function scaffoldMissing(
  projectId: string,
  dryRun: boolean,
): Promise<ProjectScaffoldMissingDto> {
  return callCommand('project_scaffold_missing', { projectId, dryRun });
}

/** One text file of a project for the viewer. */
export function readProjectFile(projectId: string, path: string): Promise<ProjectFileDto> {
  return callCommand('project_read_file', { projectId, path });
}

/** The mod folders the app knows, in scan order; the open dialog starts the picker in them. */
export async function listSources(): Promise<SourceDto[]> {
  return (await callCommand('sources_list', {})).sources;
}
