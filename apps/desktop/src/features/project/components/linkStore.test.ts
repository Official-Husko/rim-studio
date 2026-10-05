import { beforeEach, describe, expect, it } from 'vitest';
import { mockError } from 'rimstudio-testkit';
import {
  askToLink,
  cancelLink,
  confirmLink,
  linkBusy,
  linkConfirming,
  linkError,
  linkNotice,
  linkStatus,
  loadLinkStatus,
  unlink,
} from './linkStore';
import { installLinkTransport, LINK_PROJECT_ID, resultFixture, statusFixture } from './testSupport';

describe('link store', () => {
  beforeEach(() => {
    installLinkTransport();
  });

  it('reads the status of a project', async () => {
    const transport = installLinkTransport({
      project_link_status: () => statusFixture('link-status-not-linked'),
    });
    await loadLinkStatus(LINK_PROJECT_ID);
    expect(linkStatus.value?.state).toBe('not-linked');
    expect(linkBusy.value).toBe('idle');
    expect(transport.calls[0]).toEqual({
      name: 'project_link_status',
      request: { projectId: LINK_PROJECT_ID },
    });
  });

  it('keeps the error of a failed read', async () => {
    installLinkTransport({
      project_link_status: () => {
        throw mockError('project.not-open', 'project p-1 is not open');
      },
    });
    await loadLinkStatus('p-1');
    expect(linkError.value?.code).toBe('project.not-open');
    expect(linkStatus.value).toBeUndefined();
  });

  it('opens and closes the confirmation', () => {
    askToLink();
    expect(linkConfirming.value).toBe(true);
    cancelLink();
    expect(linkConfirming.value).toBe(false);
  });

  it('creates the link with the chosen mode and the running game confirmation', async () => {
    const transport = installLinkTransport({
      project_link_create: () => resultFixture('link-create-done'),
    });
    askToLink();
    await confirmLink(LINK_PROJECT_ID, 'copy', true);
    expect(transport.calls[0]).toEqual({
      name: 'project_link_create',
      request: { projectId: LINK_PROJECT_ID, mode: 'copy', confirmGameRunning: true },
    });
    expect(linkConfirming.value).toBe(false);
    expect(linkNotice.value).toEqual({ kind: 'created' });
    expect(linkStatus.value?.state).toBe('linked');
  });

  it('keeps a refusal as a notice with the reason', async () => {
    installLinkTransport({ project_link_create: () => resultFixture('link-create-refused') });
    await confirmLink(LINK_PROJECT_ID, 'symlink', false);
    expect(linkNotice.value?.kind).toBe('refused');
    expect(linkNotice.value?.refusal?.code).toBe('deploy.name-taken');
    expect(linkStatus.value?.state).toBe('foreign-folder');
    expect(linkError.value).toBeUndefined();
  });

  it('removes the link', async () => {
    installLinkTransport({ project_link_remove: () => resultFixture('link-remove-done') });
    await unlink(LINK_PROJECT_ID);
    expect(linkNotice.value).toEqual({ kind: 'removed' });
    expect(linkStatus.value?.state).toBe('not-linked');
  });

  it('drops an answer that arrives after the project changed', async () => {
    installLinkTransport({
      project_link_status: async (request) => {
        const id = (request as { projectId: string }).projectId;
        if (id === 'p-slow') await new Promise((r) => setTimeout(r, 20));
        return statusFixture('link-status-not-linked', { projectId: id });
      },
    });
    const slow = loadLinkStatus('p-slow');
    await loadLinkStatus('p-fast');
    await slow;
    expect(linkStatus.value?.projectId).toBe('p-fast');
  });
});
