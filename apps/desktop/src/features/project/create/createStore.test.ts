import { waitFor } from '@testing-library/preact';
import { beforeEach, describe, expect, it } from 'vitest';
import { installTransport } from '../testSupport';
import {
  changeForm,
  changeParent,
  createForm,
  createFolders,
  createGameVersion,
  createPreview,
  createTarget,
  createVersionChoices,
  goStep,
  identityReady,
  openCreate,
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
