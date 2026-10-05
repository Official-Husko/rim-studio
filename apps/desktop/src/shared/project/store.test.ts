import { beforeEach, describe, expect, it } from 'vitest';
import { createMockTransport, loadFixture, mockError } from 'rimstudio-testkit';
import { clearQueries, connection, setTransport } from '~/shared/ipc';
import {
  RECENT_LIMIT,
  clearRecent,
  currentProject,
  currentProjectPath,
  forgetRecent,
  getCurrentProject,
  getCurrentProjectPath,
  openProjectAt,
  recentProjects,
  resetProjectStore,
  restoreCurrentProject,
  setCurrentProject,
} from './index';

const ref = (n: number) => ({ projectId: `p-${n}`, path: `/mods/m${n}`, name: `Mod ${n}` });

beforeEach(() => {
  resetProjectStore();
  clearQueries();
  connection.value = 'mock';
});

describe('project store', () => {
  it('sets and reads the current project', () => {
    expect(getCurrentProject()).toBeUndefined();
    setCurrentProject(ref(1));
    expect(getCurrentProject()?.name).toBe('Mod 1');
    expect(getCurrentProjectPath()).toBe('/mods/m1');
    expect(currentProjectPath.value).toBe('/mods/m1');
    setCurrentProject(undefined);
    expect(currentProject.value).toBeUndefined();
  });

  it('keeps the recent list newest first without duplicates and with a limit', () => {
    for (let n = 1; n <= RECENT_LIMIT + 3; n += 1) setCurrentProject(ref(n));
    setCurrentProject(ref(5));
    const paths = recentProjects.value.map((r) => r.path);
    expect(paths).toHaveLength(RECENT_LIMIT);
    expect(paths[0]).toBe('/mods/m5');
    expect(new Set(paths).size).toBe(RECENT_LIMIT);
  });

  it('forgets one entry and clears the list', () => {
    setCurrentProject(ref(1));
    setCurrentProject(ref(2));
    forgetRecent('/mods/m1');
    expect(recentProjects.value.map((r) => r.path)).toEqual(['/mods/m2']);
    clearRecent();
    expect(recentProjects.value).toEqual([]);
  });

  it('stores the recent list in the browser', () => {
    setCurrentProject({ ...ref(1), packageId: 'a.b' });
    const raw = window.localStorage.getItem('rimstudio.project.recent');
    expect(JSON.parse(raw ?? '[]')[0]).toMatchObject({ path: '/mods/m1', packageId: 'a.b' });
  });

  it('opens a folder through the backend and makes it current', async () => {
    setTransport(
      createMockTransport({ handlers: { project_open: () => loadFixture('project-open-gewehr') } }),
    );
    const summary = await openProjectAt('/home/user/mods/[OH] Gewehr 41');
    expect(summary.packageId).toBe('oh.weapons.gewehr41');
    expect(getCurrentProject()?.projectId).toBe(summary.projectId);
    expect(recentProjects.value).toHaveLength(1);
  });

  it('restores the last project and drops a path that no longer opens', async () => {
    setTransport(
      createMockTransport({ handlers: { project_open: () => loadFixture('project-open-gewehr') } }),
    );
    window.localStorage.setItem('rimstudio.project.current', '/home/user/mods/x');
    await restoreCurrentProject();
    expect(getCurrentProject()?.name).toBeTruthy();

    resetProjectStore();
    window.localStorage.setItem('rimstudio.project.current', '/gone');
    setTransport(
      createMockTransport({
        handlers: {
          project_open: () => {
            throw mockError('io.not-found', 'not a mod project');
          },
        },
      }),
    );
    await restoreCurrentProject();
    expect(getCurrentProject()).toBeUndefined();
    expect(window.localStorage.getItem('rimstudio.project.current')).toBeNull();
  });
});
