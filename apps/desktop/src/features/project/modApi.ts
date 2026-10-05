import type {
  AboutChangeDto,
  LibraryModSearchDto,
  LoadFoldersChangeDto,
  ProjectAboutDto,
  ProjectAboutPreviewDto,
  ProjectAboutRemovePreviewDto,
  ProjectAboutSetPreviewDto,
  ProjectAboutUpdateDto,
  ProjectCreateRequest,
  ProjectLoadFoldersDto,
  ProjectLoadFoldersUpdateDto,
  ProjectScaffoldPreviewDto,
  ProjectVersionAddDto,
} from 'rimstudio-ipc-types';
import { callCommand } from '~/shared/ipc';

// One function per command of the mod basics; no state, no rules. The stores call these.

/** Every basic of the mod with the findings; the preview image as a data URL when asked. */
export function aboutGet(
  projectId: string,
  includePreviewImage: boolean,
): Promise<ProjectAboutDto> {
  return callCommand('project_about_get', { projectId, includePreviewImage });
}

/** The diff and the result of a list of changes. Writes nothing. */
export function aboutPreview(
  projectId: string,
  changes: AboutChangeDto[],
  expectedHash: string,
): Promise<ProjectAboutPreviewDto> {
  return callCommand('project_about_preview', { projectId, changes, expectedHash });
}

/** Apply the changes to About.xml as byte span edits, with a backup and a read back. */
export function aboutUpdate(
  projectId: string,
  changes: AboutChangeDto[],
  expectedHash: string,
): Promise<ProjectAboutUpdateDto> {
  return callCommand('project_about_update', { projectId, changes, expectedHash });
}

/** Copy a PNG to About/Preview.png. */
export function aboutSetPreview(
  projectId: string,
  sourcePath: string,
): Promise<ProjectAboutSetPreviewDto> {
  return callCommand('project_about_set_preview', { projectId, sourcePath });
}

/** Remove About/Preview.png after a backup. */
export function aboutRemovePreview(projectId: string): Promise<ProjectAboutRemovePreviewDto> {
  return callCommand('project_about_remove_preview', { projectId });
}

/** The blocks and entries of LoadFolders.xml. */
export function loadFoldersGet(projectId: string): Promise<ProjectLoadFoldersDto> {
  return callCommand('project_load_folders_get', { projectId });
}

/** Apply LoadFolders.xml changes; a dry run writes nothing and answers with the diff and the result. */
export function loadFoldersUpdate(
  projectId: string,
  changes: LoadFoldersChangeDto[],
  options: { expectedHash?: string; create: boolean; dryRun: boolean },
): Promise<ProjectLoadFoldersUpdateDto> {
  return callCommand('project_load_folders_update', {
    projectId,
    changes,
    create: options.create,
    dryRun: options.dryRun,
    ...(options.expectedHash ? { expectedHash: options.expectedHash } : {}),
  });
}

/** Create the folder of one more game version (and its block). */
export function versionAdd(
  projectId: string,
  version: string,
  options: { standardFolders: boolean; addBlock: boolean; dryRun: boolean },
): Promise<ProjectVersionAddDto> {
  return callCommand('project_version_add', { projectId, version, ...options });
}

/** Search the mods of the last library scan by name, package id, author and folder. */
export function searchLibrary(query: string, limit?: number): Promise<LibraryModSearchDto> {
  return callCommand('library_mod_search', { query, ...(limit ? { limit } : {}) });
}

/** What creating a new mod would write and what is wrong with the values. Writes nothing. */
export function scaffoldPreview(request: ProjectCreateRequest): Promise<ProjectScaffoldPreviewDto> {
  return callCommand('project_scaffold_preview', request);
}
