import { mockError } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { PROJECT_MISSING, withOpenProject } from './session';
import { installTransport, openFixtureProject } from './testSupport';

describe('withOpenProject', () => {
  it('passes other errors through untouched', async () => {
    installTransport();
    const p = openFixtureProject();
    await expect(
      withOpenProject({ projectId: p.projectId, path: p.path, name: p.name }, async () => {
        throw mockError('designer.internal', 'x');
      }),
    ).rejects.toMatchObject({ code: 'designer.internal' });
  });

  it('opens the folder again once and repeats the call with the new id', async () => {
    const transport = installTransport({
      project_open: () => ({
        projectId: 'p-new',
        name: 'M',
        path: '/m',
        hasAbout: true,
        hasLoadFolders: false,
        hasCeGate: false,
        supportedVersions: [],
        defFiles: 1,
        diagnostics: [],
      }),
    });
    const p = openFixtureProject();
    const seen: string[] = [];
    const out = await withOpenProject(
      { projectId: p.projectId, path: p.path, name: p.name },
      async (id) => {
        seen.push(id);
        if (id === p.projectId) throw mockError('project.not-open', 'gone');
        return 'ok';
      },
    );
    expect(out).toBe('ok');
    expect(seen).toEqual([p.projectId, 'p-new']);
    expect(transport.calls.filter((c) => c.name === 'project_open')).toHaveLength(1);
  });

  it('reports a folder that cannot be opened as missing', async () => {
    installTransport({
      project_open: () => {
        throw mockError('io.not-found', 'no such folder');
      },
    });
    const p = openFixtureProject();
    await expect(
      withOpenProject({ projectId: p.projectId, path: p.path, name: p.name }, async () => {
        throw mockError('project.not-open', 'gone');
      }),
    ).rejects.toMatchObject({ code: PROJECT_MISSING, message: 'no such folder' });
  });
});
