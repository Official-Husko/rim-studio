import type {
  AboutChangeDto,
  AboutDependencyChangeDto,
  AboutDependencyDto,
  AboutListFieldDto,
  AboutTextFieldDto,
  DiagnosticDto,
  ProjectAboutDto,
} from 'rimstudio-ipc-types';

// The draft of the About form and how it becomes the small changes the backend takes. The form keeps
// what the person typed; this module only turns the difference between the saved file and the draft
// into operations. Whether a value is acceptable is decided by the backend (the findings).

/** A dependency row of the form; `origId` is the package id it had in the file (absent for a new row). */
export interface DraftDependency extends AboutDependencyDto {
  origId?: string;
}

/** What the person changed and has not saved. */
export interface AboutDraft {
  text: Partial<Record<AboutTextFieldDto, string>>;
  lists: Partial<Record<AboutListFieldDto, string[]>>;
  deps?: DraftDependency[];
}

/** A draft with no changes. */
export function emptyDraft(): AboutDraft {
  return { text: {}, lists: {} };
}

/** The text fields in the order the form shows them. */
export const TEXT_FIELDS: readonly AboutTextFieldDto[] = [
  'name',
  'shortName',
  'author',
  'packageId',
  'modVersion',
  'url',
  'modIconPath',
  'description',
];

/** The list fields in the order the form shows them. */
export const LIST_FIELDS: readonly AboutListFieldDto[] = [
  'authors',
  'supportedVersions',
  'loadAfter',
  'loadBefore',
  'forceLoadAfter',
  'forceLoadBefore',
  'incompatibleWith',
];

const sameId = (a: string, b: string): boolean => a.toLowerCase() === b.toLowerCase();

function sameList(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((item, i) => item === b[i]);
}

/** The value of a text field: the draft when edited, else the file. */
export function textOf(
  about: ProjectAboutDto,
  draft: AboutDraft,
  field: AboutTextFieldDto,
): string {
  return draft.text[field] ?? about[field].value;
}

/** The items of a list field: the draft when edited, else the file. */
export function listOf(
  about: ProjectAboutDto,
  draft: AboutDraft,
  field: AboutListFieldDto,
): string[] {
  return draft.lists[field] ?? about[field].items;
}

/** The dependency rows: the draft when edited, else the file's, each remembering its own id. */
export function depsOf(about: ProjectAboutDto, draft: AboutDraft): DraftDependency[] {
  return draft.deps ?? about.modDependencies.map((d) => ({ ...d, origId: d.packageId }));
}

function plain(dep: DraftDependency): AboutDependencyDto {
  const out: AboutDependencyDto = { packageId: dep.packageId.trim(), displayName: dep.displayName };
  if (dep.steamWorkshopUrl) out.steamWorkshopUrl = dep.steamWorkshopUrl;
  if (dep.downloadUrl) out.downloadUrl = dep.downloadUrl;
  if (dep.alternatives && dep.alternatives.length > 0) out.alternatives = dep.alternatives;
  return out;
}

function dependencyChange(
  saved: AboutDependencyDto,
  now: DraftDependency,
): AboutDependencyChangeDto {
  const change: AboutDependencyChangeDto = {};
  if (now.packageId.trim() !== saved.packageId) change.packageId = now.packageId.trim();
  if (now.displayName !== saved.displayName) change.displayName = now.displayName;
  if ((now.steamWorkshopUrl ?? '') !== (saved.steamWorkshopUrl ?? ''))
    change.steamWorkshopUrl = now.steamWorkshopUrl ?? '';
  if ((now.downloadUrl ?? '') !== (saved.downloadUrl ?? ''))
    change.downloadUrl = now.downloadUrl ?? '';
  return change;
}

/**
 * The operations that turn the saved dependencies into the draft rows: removals, updates of the fields
 * that changed, additions, then moves until the order matches. A row with no package id yet is left out
 * (the person is still typing it).
 */
export function dependencyChanges(
  saved: readonly AboutDependencyDto[],
  rows: readonly DraftDependency[],
): AboutChangeDto[] {
  const ops: AboutChangeDto[] = [];
  const kept = rows.filter((row) => row.packageId.trim() !== '');
  const origins = new Set(
    kept.map((row) => row.origId?.toLowerCase()).filter((x) => x !== undefined),
  );
  const order: string[] = [];
  for (const dep of saved) {
    if (origins.has(dep.packageId.toLowerCase())) order.push(dep.packageId);
    else ops.push({ op: 'dependency-remove', packageId: dep.packageId });
  }
  for (const row of kept) {
    const before = row.origId
      ? saved.find((d) => sameId(d.packageId, row.origId ?? ''))
      : undefined;
    if (before) {
      const change = dependencyChange(before, row);
      if (Object.keys(change).length > 0)
        ops.push({ op: 'dependency-update', packageId: before.packageId, change });
      const at = order.findIndex((id) => sameId(id, before.packageId));
      if (at >= 0) order[at] = row.packageId.trim();
    } else {
      ops.push({ op: 'dependency-add', dependency: plain(row) });
      order.push(row.packageId.trim());
    }
  }
  const wanted = kept.map((row) => row.packageId.trim());
  wanted.forEach((id, to) => {
    if (order[to] === id) return;
    ops.push({ op: 'dependency-move', packageId: id, to });
    const from = order.findIndex((x) => sameId(x, id));
    if (from >= 0) {
      order.splice(from, 1);
      order.splice(to, 0, id);
    }
  });
  return ops;
}

/** The changes the draft makes to the saved file, in the order the backend should apply them. */
export function draftChanges(about: ProjectAboutDto, draft: AboutDraft): AboutChangeDto[] {
  const out: AboutChangeDto[] = [];
  for (const field of TEXT_FIELDS) {
    const value = draft.text[field];
    if (value === undefined || value === about[field].value) continue;
    if (value === '') {
      if (about[field].present) out.push({ op: 'clear', field });
    } else out.push({ op: 'set', field, value });
  }
  for (const field of LIST_FIELDS) {
    const items = draft.lists[field];
    if (items && !sameList(items, about[field].items)) out.push({ op: 'list-set', field, items });
  }
  if (draft.deps) out.push(...dependencyChanges(about.modDependencies, draft.deps));
  return out;
}

/** A finding pointer without its leading slash, so `/packageId` and `packageId` match. */
export function pointerOf(diagnostic: DiagnosticDto): string {
  return (diagnostic.field ?? '').replace(/^\//, '');
}

/** The findings on a field (`packageId`) or below it (`modDependencies/0/steamWorkshopUrl`). */
export function findingsFor(diagnostics: readonly DiagnosticDto[], field: string): DiagnosticDto[] {
  return diagnostics.filter((d) => {
    const at = pointerOf(d);
    return at === field || at.startsWith(`${field}/`);
  });
}

function parseVersion(text: string): [number, number] | undefined {
  const m = /^(\d+)\.(\d+)/.exec(text);
  return m ? [Number(m[1]), Number(m[2])] : undefined;
}

function compareVersions(a: string, b: string): number {
  const x = parseVersion(a);
  const y = parseVersion(b);
  if (!x || !y) return a < b ? -1 : a > b ? 1 : 0;
  return x[0] - y[0] || x[1] - y[1];
}

/** The game versions the form offers: the installed one and the three before it, plus any the file names. */
export function versionChoices(
  gameVersion: string | undefined,
  chosen: readonly string[],
): string[] {
  const set = new Set<string>(chosen);
  const game = gameVersion ? parseVersion(gameVersion) : undefined;
  if (game) {
    for (let back = 0; back < 4; back += 1) {
      if (game[1] - back >= 0) set.add(`${game[0]}.${game[1] - back}`);
    }
  } else {
    for (const v of ['1.4', '1.5', '1.6']) set.add(v);
  }
  return [...set].sort(compareVersions);
}

/** Moves one entry of a list to a new place; a copy is returned. */
export function moveItem<T>(items: readonly T[], from: number, to: number): T[] {
  const next = [...items];
  const [item] = next.splice(from, 1);
  if (item !== undefined) next.splice(to, 0, item);
  return next;
}

const INLINE_FIELDS = new Set<string>([
  'name',
  'shortName',
  'author',
  'authors',
  'packageId',
  'modVersion',
  'url',
  'modIconPath',
  'description',
  'supportedVersions',
  'modDependencies',
  'loadAfter',
  'loadBefore',
  'forceLoadAfter',
  'forceLoadBefore',
  'incompatibleWith',
]);

/** True when a section of the form shows this finding next to its field; the others go to the summary. */
export function isShownInline(diagnostic: DiagnosticDto): boolean {
  if (diagnostic.code.startsWith('about.preview')) return true;
  const first = pointerOf(diagnostic).split('/')[0] ?? '';
  return INLINE_FIELDS.has(first);
}
