import { loadFixture } from 'rimstudio-testkit';
import type { DiagnosticDto, ProjectAboutDto } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import {
  depsOf,
  draftChanges,
  emptyDraft,
  findingsFor,
  isShownInline,
  listOf,
  moveItem,
  textOf,
  versionChoices,
  type AboutDraft,
} from './aboutModel';

const about = (): ProjectAboutDto => loadFixture<ProjectAboutDto>('about-get-gewehr');

describe('draftChanges', () => {
  it('is empty for an untouched draft and for values equal to the file', () => {
    const file = about();
    expect(draftChanges(file, emptyDraft())).toEqual([]);
    const same: AboutDraft = {
      text: { name: file.name.value },
      lists: { authors: file.authors.items },
    };
    expect(draftChanges(file, same)).toEqual([]);
  });

  it('sets, clears and lists in the order of the form', () => {
    const file = about();
    const draft: AboutDraft = {
      text: { author: '', name: 'New name' },
      lists: { loadAfter: ['a.b'] },
    };
    expect(draftChanges(file, draft)).toEqual([
      { op: 'set', field: 'name', value: 'New name' },
      { op: 'clear', field: 'author' },
      { op: 'list-set', field: 'loadAfter', items: ['a.b'] },
    ]);
  });

  it('does not clear a field the file does not have', () => {
    const file = about();
    expect(draftChanges(file, { text: { shortName: '' }, lists: {} })).toEqual([]);
  });

  it('turns dependency rows into removals, updates, additions and moves', () => {
    const file = about();
    file.modDependencies = [
      { packageId: 'a.one', displayName: 'One' },
      { packageId: 'b.two', displayName: 'Two' },
      { packageId: 'c.three', displayName: 'Three' },
    ];
    const rows = depsOf(file, emptyDraft());
    const draft: AboutDraft = {
      text: {},
      lists: {},
      deps: [
        rows[2] ?? { packageId: '', displayName: '' },
        { ...(rows[0] ?? { packageId: '', displayName: '' }), displayName: 'First' },
        { packageId: 'd.four', displayName: 'Four' },
        { packageId: '', displayName: 'still typing' },
      ],
    };
    expect(draftChanges(file, draft)).toEqual([
      { op: 'dependency-remove', packageId: 'b.two' },
      { op: 'dependency-update', packageId: 'a.one', change: { displayName: 'First' } },
      { op: 'dependency-add', dependency: { packageId: 'd.four', displayName: 'Four' } },
      { op: 'dependency-move', packageId: 'c.three', to: 0 },
    ]);
  });

  it('renames a dependency by its original id', () => {
    const file = about();
    file.modDependencies = [{ packageId: 'a.one', displayName: 'One' }];
    const draft: AboutDraft = {
      text: {},
      lists: {},
      deps: [{ packageId: 'a.uno', displayName: 'One', origId: 'a.one' }],
    };
    expect(draftChanges(file, draft)).toEqual([
      { op: 'dependency-update', packageId: 'a.one', change: { packageId: 'a.uno' } },
    ]);
  });
});

describe('readers', () => {
  it('prefer the draft and fall back to the file', () => {
    const file = about();
    expect(textOf(file, emptyDraft(), 'name')).toBe(file.name.value);
    expect(textOf(file, { text: { name: 'x' }, lists: {} }, 'name')).toBe('x');
    expect(listOf(file, { text: {}, lists: { authors: ['z'] } }, 'authors')).toEqual(['z']);
    expect(listOf(file, emptyDraft(), 'authors')).toEqual(file.authors.items);
  });
});

describe('findings', () => {
  const items: DiagnosticDto[] = [
    { code: 'about.a', severity: 'error', message: 'a', field: '/packageId' },
    {
      code: 'about.b',
      severity: 'warning',
      message: 'b',
      field: '/modDependencies/1/steamWorkshopUrl',
    },
    { code: 'about.preview-missing', severity: 'warning', message: 'c' },
    { code: 'about.unparseable', severity: 'error', message: 'd' },
  ];
  it('match a field and what is below it, with or without the slash', () => {
    expect(findingsFor(items, 'packageId')).toHaveLength(1);
    expect(findingsFor(items, 'modDependencies/1')).toHaveLength(1);
    expect(findingsFor(items, 'modDependencies/10')).toHaveLength(0);
  });
  it('tell which ones a section shows next to its field', () => {
    expect(items.map(isShownInline)).toEqual([true, true, true, false]);
  });
});

describe('versionChoices and moveItem', () => {
  it('offers the installed version, three before it and what the file names', () => {
    expect(versionChoices('1.6', ['1.2'])).toEqual(['1.2', '1.3', '1.4', '1.5', '1.6']);
    expect(versionChoices(undefined, [])).toEqual(['1.4', '1.5', '1.6']);
  });
  it('moves an item without changing the original', () => {
    const list = ['a', 'b', 'c'];
    expect(moveItem(list, 0, 2)).toEqual(['b', 'c', 'a']);
    expect(list).toEqual(['a', 'b', 'c']);
  });
});
