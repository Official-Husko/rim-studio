import { beforeEach, describe, expect, it } from 'vitest';
import { loadFixture } from 'rimstudio-testkit';
import type { ProjectCreateRequest } from 'rimstudio-ipc-types';
import { currentProject } from '~/shared/project';
import {
  clearFile,
  createError,
  createMod,
  fileView,
  fixMissingFolders,
  fixResult,
  loadError,
  loadProject,
  loadState,
  openError,
  openFolder,
  selectedPath,
  showFile,
  view,
} from './store';
import { gewehrRef, installTransport } from './testSupport';

beforeEach(() => {
  installTransport();
});

describe('loadProject', () => {
  it('reads the summary, the tree and the check', async () => {
    await loadProject(gewehrRef());
    expect(loadState.value).toBe('ready');
    expect(view.value?.summary.name).toBe("Huskos's Gewehr 41");
    expect(view.value?.tree.counts.weaponDefs).toBe(5);
    expect(view.value?.check.autoFixable).toBe(2);
  });

  it('keeps an error and the state', async () => {
    installTransport({
      project_tree: () => {
        throw { code: 'project.not-open', message: 'not open', errorId: 'e-2' };
      },
    });
    await loadProject(gewehrRef());
    expect(loadState.value).toBe('error');
    expect(loadError.value?.code).toBe('project.not-open');
    expect(view.value).toBeUndefined();
  });

  it('drops the selected file when another project is loaded', async () => {
    await loadProject(gewehrRef());
    await showFile('About/About.xml');
    expect(fileView.value?.file?.path).toBe('About/About.xml');
    await loadProject({ projectId: 'p-5f53fa51', path: '/x/Lone Wolf', name: 'Lone' });
    expect(selectedPath.value).toBeUndefined();
    expect(fileView.value).toBeUndefined();
  });
});

describe('showFile', () => {
  it('shows a text file, a binary file and an error', async () => {
    await loadProject(gewehrRef());
    await showFile('Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml');
    expect(fileView.value?.file?.truncated).toBe(true);
    await showFile('About/Preview.png');
    expect(fileView.value?.file?.binary).toBe(true);
    installTransport({
      project_read_file: () => {
        throw { code: 'project.path-outside-root', message: 'refused', errorId: 'e-3' };
      },
    });
    await loadProject(gewehrRef());
    await showFile('../x');
    expect(fileView.value?.error?.code).toBe('project.path-outside-root');
    clearFile();
    expect(selectedPath.value).toBeUndefined();
  });

  it('ignores a file request without a loaded project', async () => {
    await showFile('About/About.xml');
    expect(fileView.value).toBeUndefined();
  });
});

describe('openFolder and createMod', () => {
  it('opens a folder and makes it current', async () => {
    expect(await openFolder('/home/user/mods/[OH] Gewehr 41')).toBe(true);
    expect(currentProject.value?.packageId).toBe('oh.weapons.gewehr41');
  });

  it('keeps the error of a folder that is not a mod', async () => {
    installTransport({
      project_open: () => {
        throw { code: 'io.not-found', message: 'not a mod project', errorId: 'e-4' };
      },
    });
    expect(await openFolder('/nope')).toBe(false);
    expect(openError.value?.code).toBe('io.not-found');
  });

  it('creates a mod and makes it current, or keeps the backend error', async () => {
    const request = { path: '/home/user/mods/X', name: 'X' } as ProjectCreateRequest;
    expect(await createMod(request)).toBe(true);
    expect(currentProject.value?.name).toBe(
      loadFixture<{ name: string }>('project-create-new').name,
    );
    installTransport({
      project_create: () => {
        throw { code: 'designer.apply-failed', message: 'the mod name is empty', errorId: 'e-5' };
      },
    });
    expect(await createMod(request)).toBe(false);
    expect(createError.value?.message).toBe('the mod name is empty');
  });
});

describe('fixMissingFolders', () => {
  it('creates the folders and reads the project again', async () => {
    const transport = installTransport();
    await loadProject(gewehrRef());
    await fixMissingFolders();
    expect(fixResult.value?.folders).toEqual(['Defs/SoundDefs', 'Sounds']);
    expect(transport.calls.filter((c) => c.name === 'project_tree')).toHaveLength(2);
  });
});
