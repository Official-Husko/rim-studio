import { waitFor } from '@testing-library/preact';
import { loadFixture } from 'rimstudio-testkit';
import type { ProjectScaffoldPreviewDto } from 'rimstudio-ipc-types';
import { beforeEach, describe, expect, it } from 'vitest';
import { installTransport } from '../testSupport';
import {
  changeForm,
  changeParent,
  createChecking,
  createForm,
  createFolders,
  createGameVersion,
  createPreview,
  createTarget,
  createVersionChoices,
  goStep,
  identityReady,
  openCreate,
  scheduleCheck,
  toggleVersion,
} from './createStore';

beforeEach(() => {
  installTransport();
  openCreate('/mods');
});

describe('createStore', () => {
  it('follows the name and author until the package id and folder are typed', () => {
    changeForm({ name: 'Arms Pack', author: 'Me' });
    expect(createForm.value.packageId).toBe('me.armspack');
    expect(createForm.value.folderName).toBe('Arms Pack');
    changeForm({ packageId: 'custom.id' });
    changeForm({ name: 'Other' });
    expect(createForm.value.packageId).toBe('custom.id');
    expect(createTarget.value).toBe('/mods/Other');
  });

  it('asks the backend after a pause and becomes ready on a valid answer', async () => {
    const transport = installTransport();
    openCreate('/mods');
    changeForm({ name: 'Arms Pack', author: 'Me' });
    expect(identityReady.value).toBe(false);
    await waitFor(() => expect(createPreview.value?.valid).toBe(true));
    await waitFor(() => expect(identityReady.value).toBe(true));
    expect(transport.calls.filter((c) => c.name === 'project_scaffold_preview')).toHaveLength(1);
  });

  it('drops the answer of a check that was in flight when the form changed', async () => {
    const answers: Array<(value: ProjectScaffoldPreviewDto) => void> = [];
    installTransport({
      project_scaffold_preview: () =>
        new Promise<ProjectScaffoldPreviewDto>((resolve) => {
          answers.push(resolve);
        }),
    });
    openCreate('/mods');
    changeForm({ name: 'Arms Pack', author: 'Me' });
    scheduleCheck(true);
    await waitFor(() => expect(answers).toHaveLength(1));
    // the person types again while the first answer is on its way
    changeForm({ name: 'Arms Pack Two' });
    answers[0]?.(loadFixture<ProjectScaffoldPreviewDto>('scaffold-preview-versioned'));
    // long enough for the answer to be handled, short enough that the new check has not started
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(createPreview.value).toBeUndefined();
    expect(createChecking.value).toBe(true);
    expect(identityReady.value).toBe(false);
    await waitFor(() => expect(answers).toHaveLength(2));
    answers[1]?.(loadFixture<ProjectScaffoldPreviewDto>('scaffold-preview-versioned'));
    await waitFor(() => expect(createChecking.value).toBe(false));
    expect(createPreview.value).toBeDefined();
  });

  it('needs a folder to put the mod in', async () => {
    changeParent('');
    changeForm({ name: 'Arms Pack', author: 'Me' });
    expect(createTarget.value).toBe('');
    expect(identityReady.value).toBe(false);
  });

  it('asks at once when the review step opens', async () => {
    const transport = installTransport();
    openCreate('/mods');
    changeForm({ name: 'Arms Pack', author: 'Me' });
    goStep('review');
    await waitFor(() => expect(createPreview.value).toBeDefined());
    expect(transport.calls.some((c) => c.name === 'project_scaffold_preview')).toBe(true);
  });

  it('offers the detected version and its neighbours and ticks versions in order', async () => {
    installTransport({
      detect_get_report: () => ({
        report: { installs: [{ version: { raw: '1.5.1', major: 1, minor: 5 } }] },
      }),
    });
    openCreate('/mods');
    await waitFor(() => expect(createGameVersion.value).toBe('1.5'));
    expect(createVersionChoices.value).toEqual(['1.2', '1.3', '1.4', '1.5']);
    toggleVersion('1.3', true);
    expect(createForm.value.versions).toEqual(['1.3', '1.5']);
    toggleVersion('1.5', false);
    expect(createForm.value.versions).toEqual(['1.3']);
  });

  it('keeps the mod folders of Setup for quick choices', async () => {
    await waitFor(() => expect(createFolders.value.length).toBeGreaterThan(0));
    expect(createFolders.value.every((s) => s.kind === 'custom' || s.kind === 'game-mods')).toBe(
      true,
    );
  });
});
