import { createMockTransport, loadFixture, type MockHandler } from 'rimstudio-testkit';
import type { ProjectSummaryDto } from 'rimstudio-ipc-types';
import { clearQueries, connection, setTransport } from '~/shared/ipc';
import { resetProjectStore, setCurrentProject, type ProjectRef } from '~/shared/project';
import { resetProjectView } from './store';
import { resetAboutStore } from './basics/aboutStore';
import { resetCreateStore } from './create/createStore';
import { resetFoldersStore } from './folders/folderStore';

/** The two recorded projects: the game style Gewehr 41 and the flat Lone Wolf package. */
export const GEWEHR_ID = 'p-c36c596a';
export const LONE_WOLF_ID = 'p-5f53fa51';

/** The reference of the recorded Gewehr 41 project. */
export function gewehrRef(): ProjectRef {
  const summary = loadFixture<ProjectSummaryDto>('project-open-gewehr');
  return {
    projectId: summary.projectId,
    path: summary.path,
    name: summary.name,
    ...(summary.packageId ? { packageId: summary.packageId } : {}),
  };
}

function idOf(request: unknown): string {
  return (request as { projectId: string }).projectId;
}

/** Handlers answering the project commands from the fixtures recorded with the real backend. */
export function projectHandlers(): Record<string, MockHandler> {
  return {
    project_open: (request) =>
      (request as { path: string }).path.includes('Lone Wolf')
        ? loadFixture('project-open-lonewolf')
        : loadFixture('project-open-gewehr'),
    project_tree: (request) =>
      loadFixture(idOf(request) === LONE_WOLF_ID ? 'project-tree-lonewolf' : 'project-tree-gewehr'),
    project_layout_check: (request) =>
      loadFixture(idOf(request) === LONE_WOLF_ID ? 'layout-check-lonewolf' : 'layout-check-gewehr'),
    project_read_file: (request) => {
      const path = (request as { path: string }).path;
      if (path.endsWith('.png')) return loadFixture('project-file-binary');
      if (path.endsWith('.xml') && path.startsWith('Defs'))
        return loadFixture('project-file-weapons');
      return loadFixture('project-file-about');
    },
    project_scaffold_missing: () => ({
      projectId: GEWEHR_ID,
      dryRun: false,
      folders: ['Defs/SoundDefs', 'Sounds'],
      files: [],
      skipped: [],
    }),
    project_layout_fix_plan: (request) =>
      loadFixture(
        idOf(request) === LONE_WOLF_ID ? 'layout-fix-plan-lonewolf' : 'layout-fix-plan-gewehr',
      ),
    project_layout_fix_apply: () => loadFixture('layout-fix-apply-gewehr'),
    project_layout_fix_undo: () => loadFixture('layout-fix-undo-gewehr'),
    project_layout_fix_history: () => loadFixture('layout-fix-history-gewehr'),
    project_create: () => loadFixture('project-create-new'),
    sources_list: () => loadFixture('sources-list-default'),
  };
}

/** Swap the mock transport without touching the stores; extra handlers win over the fixture ones. */
export function replaceTransport(extra: Record<string, MockHandler> = {}) {
  clearQueries();
  const transport = createMockTransport({
    handlers: { ...projectHandlers(), ...modHandlers(), ...extra },
  });
  setTransport(transport);
  connection.value = 'mock';
  return transport;
}

/** Reset the stores and install a mock transport for a test. */
export function installTransport(extra: Record<string, MockHandler> = {}) {
  resetProjectStore();
  resetProjectView();
  resetModStores();
  return replaceTransport(extra);
}

/** Install the transport and make the Gewehr 41 project current. */
export function installWithProject(extra: Record<string, MockHandler> = {}) {
  const transport = installTransport(extra);
  setCurrentProject(gewehrRef());
  return transport;
}

/** Handlers answering the mod basics commands from the fixtures recorded with the real backend. */
export function modHandlers(): Record<string, MockHandler> {
  return {
    project_about_get: () => loadFixture('about-get-gewehr'),
    project_about_preview: () => loadFixture('about-preview-gewehr'),
    project_about_update: () => loadFixture('about-update-gewehr'),
    project_about_set_preview: () => ({
      preview: { exists: true, path: 'About/Preview.png', bytes: 1024, width: 640, height: 360 },
      replaced: false,
      diagnostics: [],
    }),
    project_about_remove_preview: () => ({ removed: true }),
    project_load_folders_get: () => loadFixture('load-folders-get-none'),
    project_load_folders_update: () => loadFixture('load-folders-update-dry'),
    project_version_add: () => loadFixture('version-add-dry'),
    library_mod_search: () => loadFixture('library-search-combat'),
    project_scaffold_preview: () => loadFixture('scaffold-preview-versioned'),
    detect_get_report: () => ({}),
  };
}

/** Forget the state of the three stores of the mod basics. */
export function resetModStores(): void {
  resetAboutStore();
  resetFoldersStore();
  resetCreateStore();
}
