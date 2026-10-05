import { loadFixture } from 'rimstudio-testkit';
import type { ProjectAboutDto, ProjectAboutUpdateDto } from 'rimstudio-ipc-types';
import { beforeEach, describe, expect, it } from 'vitest';
import { waitFor } from '@testing-library/preact';
import { projectRevision } from '~/shared/project';
import { installTransport } from '../testSupport';
import {
  aboutDraft,
  aboutModel,
  aboutPreviewResult,
  aboutSaving,
  aboutStale,
  discardAbout,
  findings,
  imageNotice,
  loadAbout,
  pendingChanges,
  removePreviewImage,
  saveAbout,
  saveError,
  setDependencies,
  setList,
  setPreviewImage,
  setText,
} from './aboutStore';

const ID = 'p-c36c596a';

beforeEach(() => {
  installTransport();
});

describe('aboutStore', () => {
  it('reads the file and starts with no changes', async () => {
    await loadAbout(ID);
    expect(aboutModel.value?.packageId.value).toBe('oh.weapons.gewehr41');
    expect(pendingChanges.value).toEqual([]);
    expect(findings.value.map((d) => d.code)).toContain('about.supported-versions-missing-game');
  });

  it('previews an edit after a pause and shows the findings of the result', async () => {
    const transport = installTransport();
    await loadAbout(ID);
    setText('name', 'Another name');
    expect(pendingChanges.value).toEqual([{ op: 'set', field: 'name', value: 'Another name' }]);
    await waitFor(() => expect(aboutPreviewResult.value).toBeDefined());
    const call = transport.calls.find((c) => c.name === 'project_about_preview');
    expect(call?.request).toMatchObject({
      projectId: ID,
      expectedHash: aboutModel.value?.fileHash,
      changes: [{ op: 'set', field: 'name', value: 'Another name' }],
    });
    expect(findings.value).toEqual(aboutPreviewResult.value?.result.diagnostics);
  });

  it('drops the preview when the edit is undone', async () => {
    await loadAbout(ID);
    setText('name', 'x');
    await waitFor(() => expect(aboutPreviewResult.value).toBeDefined());
    setText('name', aboutModel.value?.name.value ?? '');
    await waitFor(() => expect(aboutPreviewResult.value).toBeUndefined());
    expect(pendingChanges.value).toEqual([]);
  });

  it('saves with the file hash, clears the draft and tells the project to reload', async () => {
    const transport = installTransport();
    await loadAbout(ID);
    const before = projectRevision.value;
    setText('name', 'Saved name');
    expect(await saveAbout()).toBe(true);
    const call = transport.calls.find((c) => c.name === 'project_about_update');
    expect(call?.request).toMatchObject({ projectId: ID, expectedHash: expect.any(String) });
    expect(aboutDraft.value.text).toEqual({});
    expect(aboutModel.value?.name.value).toBe(
      loadFixture<ProjectAboutUpdateDto>('about-update-gewehr').about.name.value,
    );
    expect(projectRevision.value).toBe(before + 1);
    expect(aboutSaving.value).toBe(false);
  });

  it('explains a stale file and keeps the edits', async () => {
    installTransport({
      project_about_update: () => {
        throw loadFixture<{ error: unknown }>('about-update-stale').error;
      },
    });
    await loadAbout(ID);
    setText('name', 'Mine');
    expect(await saveAbout()).toBe(false);
    expect(aboutStale.value).toBe(true);
    expect(saveError.value?.code).toBe('project.file-stale');
    expect(pendingChanges.value).toHaveLength(1);
    // reading again keeps the draft
    await loadAbout(ID, true);
    expect(aboutStale.value).toBe(false);
    expect(saveError.value).toBeUndefined();
    expect(pendingChanges.value).toHaveLength(1);
  });

  it('keeps list and dependency edits as one operation set and discards them', async () => {
    await loadAbout(ID);
    setList('authors', ['A', 'B']);
    setDependencies([{ packageId: 'x.y', displayName: 'X' }]);
    expect(pendingChanges.value.map((c) => c.op)).toEqual(['list-set', 'dependency-add']);
    discardAbout();
    expect(pendingChanges.value).toEqual([]);
    expect(aboutPreviewResult.value).toBeUndefined();
  });

  it('resets when another project is loaded', async () => {
    await loadAbout(ID);
    setText('name', 'x');
    await loadAbout('p-other');
    expect(pendingChanges.value).toEqual([]);
  });

  it('copies and removes the preview image and reads the file again', async () => {
    const transport = installTransport();
    await loadAbout(ID);
    await setPreviewImage('/tmp/pic.png');
    expect(transport.calls.find((c) => c.name === 'project_about_set_preview')?.request).toEqual({
      projectId: ID,
      sourcePath: '/tmp/pic.png',
    });
    expect(imageNotice.value).toBe('added');
    await removePreviewImage();
    expect(imageNotice.value).toBe('removed');
    expect(transport.calls.filter((c) => c.name === 'project_about_get').length).toBeGreaterThan(2);
  });

  it('keeps an unparsed file shown but not editable', async () => {
    const locked = { ...loadFixture<ProjectAboutDto>('about-get-gewehr'), editable: false };
    installTransport({ project_about_get: () => locked });
    await loadAbout(ID);
    expect(aboutModel.value?.editable).toBe(false);
  });
});
