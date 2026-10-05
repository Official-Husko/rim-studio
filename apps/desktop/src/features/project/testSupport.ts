import { createMockTransport, loadFixture, type MockHandler } from 'rimstudio-testkit';
import type { ProjectSummaryDto } from 'rimstudio-ipc-types';
import { clearQueries, connection, setTransport } from '~/shared/ipc';
import { resetProjectStore, setCurrentProject, type ProjectRef } from '~/shared/project';
import { resetProjectView } from './store';

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
    project_create: () => loadFixture('project-create-new'),
    sources_list: () => loadFixture('sources-list-default'),
  };
}

/** Swap the mock transport without touching the stores; extra handlers win over the fixture ones. */
export function replaceTransport(extra: Record<string, MockHandler> = {}) {
  clearQueries();
  const transport = createMockTransport({ handlers: { ...projectHandlers(), ...extra } });
  setTransport(transport);
  connection.value = 'mock';
  return transport;
}

/** Reset the stores and install a mock transport for a test. */
export function installTransport(extra: Record<string, MockHandler> = {}) {
  resetProjectStore();
  resetProjectView();
  return replaceTransport(extra);
}

/** Install the transport and make the Gewehr 41 project current. */
export function installWithProject(extra: Record<string, MockHandler> = {}) {
  const transport = installTransport(extra);
  setCurrentProject(gewehrRef());
  return transport;
}
