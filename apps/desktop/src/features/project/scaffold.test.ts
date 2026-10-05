import { describe, expect, it } from 'vitest';
import { loadFixture } from 'rimstudio-testkit';
import type { ProjectTreeDto, TreeNodeDto } from 'rimstudio-ipc-types';
import { emptyForm, requestOf, scaffoldPreview, type NewModForm } from './scaffold';

function paths(node: TreeNodeDto, out: string[] = []): string[] {
  for (const child of node.children) {
    out.push(child.path);
    paths(child, out);
  }
  return out;
}

function preview(patch: Partial<NewModForm>): string[] {
  const form = { ...emptyForm(), ...patch };
  return scaffoldPreview(form).map((e) => e.path);
}

const recorded = (name: string): string[] => paths(loadFixture<ProjectTreeDto>(name).root).sort();

describe('scaffoldPreview', () => {
  // The fixtures are trees recorded from folders the real backend scaffolded.
  it('matches the default scaffold', () => {
    expect(preview({})).toEqual(recorded('project-tree-new'));
  });

  it('matches a scaffold with no optional folders', () => {
    expect(preview({ patchesFolder: false, texturesFolder: false, soundsFolder: false })).toEqual(
      recorded('scaffold-tree-bare'),
    );
  });

  it('matches the gated Combat Extended folder with source, credits and ignore file', () => {
    expect(
      preview({ cePatchFolder: true, sourceFolder: true, gitignore: true, credits: true }),
    ).toEqual(recorded('scaffold-tree-ce'));
  });

  it('matches the versioned layout', () => {
    expect(preview({ versionedFolders: true, versions: '1.5, 1.6' })).toEqual(
      recorded('scaffold-tree-versioned'),
    );
  });

  it('matches everything switched on', () => {
    expect(
      preview({
        versionedFolders: true,
        versions: '1.5,1.6',
        languagesFolder: true,
        assembliesFolder: true,
        cePatchFolder: true,
        sourceFolder: true,
        gitignore: true,
        ignoreSourceArt: true,
        readme: true,
        credits: true,
        placeholderFiles: true,
      }),
    ).toEqual(recorded('scaffold-tree-full'));
  });
});

describe('requestOf', () => {
  it('trims the text and splits the versions', () => {
    const request = requestOf(
      { ...emptyForm(), name: ' Arsenal ', packageId: ' a.b ', versions: '1.5, 1.6' },
      '/mods/Arsenal',
    );
    expect(request).toMatchObject({
      path: '/mods/Arsenal',
      name: 'Arsenal',
      packageId: 'a.b',
      supportedVersions: ['1.5', '1.6'],
    });
  });

  it('ignores the source art option without a source folder', () => {
    expect(requestOf({ ...emptyForm(), ignoreSourceArt: true }, '/x').ignoreSourceArt).toBe(false);
  });
});
