import { describe, expect, it } from 'vitest';
import { loadFixture } from 'rimstudio-testkit';
import type { ProjectTreeDto, TreeNodeDto } from 'rimstudio-ipc-types';
import {
  ROOT_ID,
  ancestorsOf,
  extensionOf,
  findNode,
  folderNameOf,
  iconOf,
  idOf,
  initiallyOpen,
  issuesUnder,
  joinPath,
  splitVersions,
  suggestPackageId,
  worstSeverity,
} from './model';

const tree = loadFixture<ProjectTreeDto>('project-tree-gewehr');

describe('tree helpers', () => {
  it('finds a node by path and the root by the empty path', () => {
    expect(findNode(tree.root, '')?.name).toBe('[OH] Gewehr 41');
    expect(findNode(tree.root, 'Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml')?.kind).toBe(
      'file',
    );
    expect(findNode(tree.root, 'Defs/Nope')).toBeUndefined();
  });

  it('gives the root a non empty tree id', () => {
    expect(idOf(tree.root)).toBe(ROOT_ID);
    expect(idOf({ path: 'About' })).toBe('About');
  });

  it('lists the folders above a path, outermost first', () => {
    expect(ancestorsOf('a/b/c.xml')).toEqual(['', 'a', 'a/b']);
    expect(ancestorsOf('c.xml')).toEqual(['']);
  });

  it('opens the root and the content folders, not source or other', () => {
    const open = initiallyOpen(tree.root);
    expect(open).toContain(ROOT_ID);
    expect(open).toContain('Defs');
    expect(open).not.toContain('Source');
    expect(open).not.toContain('Config');
  });

  it('chooses an icon by role and by file kind', () => {
    const node = (name: string, kind: TreeNodeDto['kind'], role: TreeNodeDto['role']) =>
      ({
        name,
        path: name,
        kind,
        role,
        bytes: 0,
        files: 0,
        issues: 0,
        children: [],
      }) as TreeNodeDto;
    expect(iconOf(node('Patches', 'folder', 'patches'))).toBe('patch');
    expect(iconOf(node('a.xml', 'file', 'other'))).toBe('xml');
    expect(iconOf(node('a.PNG', 'file', 'other'))).toBe('image');
    expect(iconOf(node('a.ogg', 'file', 'other'))).toBe('sound');
    expect(iconOf(node('notes', 'file', 'other'))).toBe('file');
    expect(extensionOf('.gitignore')).toBe('');
  });
});

describe('issue helpers', () => {
  const issues = tree.issues;
  it('finds the worst severity', () => {
    expect(worstSeverity([])).toBeUndefined();
    expect(worstSeverity([{ severity: 'info' }, { severity: 'warning' }])).toBe('warning');
    expect(worstSeverity([{ severity: 'hint' }, { severity: 'error' }])).toBe('error');
  });

  it('selects the issues on or below a path', () => {
    expect(issuesUnder(issues, '')).toHaveLength(issues.length);
    expect(issuesUnder(issues, 'Patches').every((i) => i.path.startsWith('Patches'))).toBe(true);
    expect(issuesUnder(issues, 'Patch')).toEqual([]);
  });
});

describe('form helpers', () => {
  it('suggests a package id from author and name', () => {
    expect(suggestPackageId('Pawbeans', 'Pawbeans Arsenal')).toBe('pawbeans.pawbeansarsenal');
    expect(suggestPackageId('', 'Mod One')).toBe('modone');
    expect(suggestPackageId('Héllo', '')).toBe('hello');
    expect(suggestPackageId('', '')).toBe('');
  });

  it('cleans a folder name', () => {
    expect(folderNameOf('A: B/C? ')).toBe('A BC');
    expect(folderNameOf('name. ')).toBe('name');
  });

  it('joins a path with the separator of the parent', () => {
    expect(joinPath('/home/user', 'Mod')).toBe('/home/user/Mod');
    expect(joinPath('/home/user/', 'Mod')).toBe('/home/user/Mod');
    expect(joinPath('C:\\Mods', 'Mod')).toBe('C:\\Mods\\Mod');
  });

  it('splits typed versions', () => {
    expect(splitVersions('1.5, 1.6')).toEqual(['1.5', '1.6']);
    expect(splitVersions(' 1.6 ;; ')).toEqual(['1.6']);
    expect(splitVersions('')).toEqual([]);
  });
});
