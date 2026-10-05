import { loadFixture } from 'rimstudio-testkit';
import type { ProjectLoadFoldersDto } from 'rimstudio-ipc-types';
import { beforeEach, describe, expect, it } from 'vitest';
import { projectRevision } from '~/shared/project';
import { installTransport } from '../testSupport';
import {
  applyFoldersOp,
  discardFolders,
  foldersEditError,
  foldersModel,
  foldersOps,
  foldersPending,
  foldersPreview,
  foldersShown,
  foldersStale,
  loadFolders,
  saveFolders,
  startFoldersCreate,
} from './folderStore';

const ID = 'p-c36c596a';
const versioned = () => loadFixture<ProjectLoadFoldersDto>('load-folders-get-versioned');

function use(extra = {}) {
  return installTransport({ project_load_folders_get: () => versioned(), ...extra });
}

beforeEach(() => {
  installTransport();
});

describe('folderStore', () => {
  it('reads the blocks and starts with no edits', async () => {
    use();
    await loadFolders(ID);
    expect(foldersModel.value?.blocks).toHaveLength(2);
    expect(foldersPending.value).toBe(0);
    expect(foldersShown.value?.exists).toBe(true);
  });

  it('checks an edit with a dry run and shows the result', async () => {
    const transport = use();
    await loadFolders(ID);
    const ok = await applyFoldersOp({ op: 'remove-block', block: 0 });
    expect(ok).toBe(true);
    const call = transport.calls.find((c) => c.name === 'project_load_folders_update');
    expect(call?.request).toMatchObject({
      projectId: ID,
      dryRun: true,
      create: false,
      expectedHash: versioned().fileHash,
      changes: [{ op: 'remove-block', block: 0 }],
    });
    expect(foldersPending.value).toBe(1);
    expect(foldersPreview.value?.diff).toContain('LoadFolders.xml');
  });

  it('drops an edit the backend refuses and says why', async () => {
    use({
      project_load_folders_update: () => {
        throw { code: 'project.edit-invalid', message: 'no such block', errorId: 'e-1' };
      },
    });
    await loadFolders(ID);
    expect(await applyFoldersOp({ op: 'remove-block', block: 9 })).toBe(false);
    expect(foldersOps.value).toEqual([]);
    expect(foldersEditError.value?.message).toBe('no such block');
  });

  it('saves for real with the hash and tells the project to reload', async () => {
    const transport = use();
    await loadFolders(ID);
    await applyFoldersOp({ op: 'remove-block', block: 0 });
    const before = projectRevision.value;
    expect(await saveFolders()).toBe(true);
    const last = [...transport.calls]
      .reverse()
      .find((c) => c.name === 'project_load_folders_update');
    expect(last?.request).toMatchObject({ dryRun: false });
    expect(foldersPending.value).toBe(0);
    expect(projectRevision.value).toBe(before + 1);
  });

  it('creates the file only when asked, and counts that as one change', async () => {
    const transport = installTransport();
    await loadFolders(ID);
    expect(foldersModel.value?.exists).toBe(false);
    await startFoldersCreate();
    expect(foldersPending.value).toBe(1);
    expect(
      transport.calls.find((c) => c.name === 'project_load_folders_update')?.request,
    ).toMatchObject({
      create: true,
      dryRun: true,
    });
  });

  it('marks the file stale and discards edits', async () => {
    use({
      project_load_folders_update: () => {
        throw { code: 'project.file-stale', message: 'changed', errorId: 'e-2' };
      },
    });
    await loadFolders(ID);
    await applyFoldersOp({ op: 'remove-block', block: 0 });
    expect(foldersStale.value).toBe(true);
    discardFolders();
    expect(foldersPending.value).toBe(0);
  });
});
