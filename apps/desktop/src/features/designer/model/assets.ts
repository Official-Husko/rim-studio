import type {
  AssetImportsDto,
  DiagnosticDto,
  CustomSoundDto,
  DesignSpecDto,
  FloatRangeDto,
  PlannedFileDto,
  SoundImportsDto,
  WritePlanDto,
} from 'rimstudio-ipc-types';
import type { FileFilter } from '~/shared/platform';
import { getAt } from './pointer';

// Pure helpers of the texture and sound panels: they read the draft and the plan for display and
// build the value a field change writes. Validation, limits and paths all come from the backend.

/** The file types the texture picker offers. */
export const TEXTURE_FILTER: FileFilter[] = [{ name: 'PNG image', extensions: ['png'] }];

/** The file types the clip picker offers. */
export const CLIP_FILTER: FileFilter[] = [{ name: 'WAV or Ogg sound', extensions: ['wav', 'ogg'] }];

/** The two texture slots of a weapon. */
export type TextureSlot = 'texture' | 'projectileTexture';

/** Where the picture a slot shows comes from. */
export type TextureOrigin =
  | { kind: 'imported'; target?: string }
  | { kind: 'shared'; texPath: string }
  | { kind: 'reserved' };

/** The imports of the draft with one slot changed; undefined when nothing is left to import. */
export function assetsWith(
  spec: DesignSpecDto,
  slot: TextureSlot,
  path: string | undefined,
): AssetImportsDto | undefined {
  const next: AssetImportsDto = { ...spec.assets };
  if (path === undefined) delete next[slot];
  else next[slot] = path;
  return Object.keys(next).length === 0 ? undefined : next;
}

/** The custom shot sound of a draft. */
export function shotOf(spec: DesignSpecDto): CustomSoundDto | undefined {
  return spec.sounds?.shot;
}

/** The sounds of the draft with the shot replaced; undefined when no custom sound is left. */
export function soundsWith(
  spec: DesignSpecDto,
  shot: CustomSoundDto | undefined,
): SoundImportsDto | undefined {
  if (shot === undefined) {
    const rest: SoundImportsDto = { ...spec.sounds };
    delete rest.shot;
    return Object.keys(rest).length === 0 ? undefined : rest;
  }
  return { ...spec.sounds, shot };
}

/** A custom sound with one member changed; an undefined value or an empty list removes the member. */
export function shotWith<K extends keyof CustomSoundDto>(
  shot: CustomSoundDto | undefined,
  key: K,
  value: CustomSoundDto[K] | undefined,
): CustomSoundDto {
  const next: CustomSoundDto = { ...shot };
  if (value === undefined || (Array.isArray(value) && value.length === 0)) delete next[key];
  else next[key] = value;
  return next;
}

/** A range from two ends: both present make a range, both empty remove it, one alone is not yet one. */
export function rangeOf(
  min: number | undefined,
  max: number | undefined,
): FloatRangeDto | undefined | 'incomplete' {
  if (min === undefined && max === undefined) return undefined;
  if (min === undefined || max === undefined) return 'incomplete';
  return { min, max };
}

/** True when the weapon has a projectile of its own (the only projectile that can carry art). */
export function hasOwnProjectile(spec: DesignSpecDto): boolean {
  return spec.ranged?.projectile?.mode === 'inline';
}

/** The copied files of a plan. */
export function copiesOf(plan: WritePlanDto | undefined): PlannedFileDto[] {
  return plan?.files.filter((f) => f.kind === 'copy') ?? [];
}

/** The planned copy of a source file, if the plan has one. */
export function copyOfSource(
  plan: WritePlanDto | undefined,
  source: string,
): PlannedFileDto | undefined {
  return copiesOf(plan).find((f) => f.copy?.source === source);
}

/** The `texPath` a planned texture copy sets: its path without the Textures folder and extension. */
export function texPathOf(copyPath: string): string {
  return copyPath.replace(/^(?:.*\/)?Textures\//, '').replace(/\.png$/i, '');
}

/** Where the picture of a slot comes from, for the line under its title. */
export function textureOrigin(
  spec: DesignSpecDto,
  slot: TextureSlot,
  plan: WritePlanDto | undefined,
): TextureOrigin {
  const imported = spec.assets?.[slot];
  if (imported !== undefined) {
    const copy = copyOfSource(plan, imported);
    return copy ? { kind: 'imported', target: texPathOf(copy.path) } : { kind: 'imported' };
  }
  const pointer = slot === 'texture' ? '/texturePath' : '/ranged/projectile/def/texturePath';
  const typed = getAt(spec, pointer);
  return typeof typed === 'string' && typed !== ''
    ? { kind: 'shared', texPath: typed }
    : { kind: 'reserved' };
}

/** True for a planned file of sound definitions (display only). */
export function isSoundDefFile(file: PlannedFileDto): boolean {
  return file.kind === 'vanilla-defs' && /(^|\/)SoundDefs\//.test(file.path);
}

/** The planned clip copies, in plan order. */
export function clipCopies(plan: WritePlanDto | undefined): PlannedFileDto[] {
  return copiesOf(plan).filter((f) => /(^|\/)Sounds\//.test(f.path));
}

/** The folder and name of a clip as `clipPath` writes it: no Sounds folder, no extension. */
function clipKey(copyPath: string): string {
  return copyPath.replace(/^(?:.*\/)?Sounds\//, '').replace(/\.[^./]+$/, '');
}

/**
 * The name of the sound definition the plan writes for the planned clips: the `SoundDef` block of the
 * sound file that names one of the clips. This reads the generated text for display only.
 */
export function plannedSoundName(plan: WritePlanDto | undefined): string | undefined {
  const file = plan?.files.find(isSoundDefFile);
  const keys = new Set(clipCopies(plan).map((f) => clipKey(f.path)));
  if (!file || keys.size === 0) return undefined;
  for (const block of file.rendered.match(/<SoundDef>[\s\S]*?<\/SoundDef>/g) ?? []) {
    const name = /<defName>([^<]+)<\/defName>/.exec(block)?.[1];
    const clips = [...block.matchAll(/<clipPath>([^<]+)<\/clipPath>/g)].map((m) => m[1] ?? '');
    if (name && clips.some((c) => keys.has(c))) return name.trim();
  }
  return undefined;
}

/** The first characters of a hash, enough to tell two files apart. */
export function shortHash(sha256: string): string {
  return sha256.slice(0, 12);
}

/** The last part of a path, whatever the separator. */
export function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/**
 * The problems of the imports the plan found (the plan reads the files; the preview does not): the
 * ones on the texture and sound imports, and the note that a custom shot replaces a named sound.
 */
export function importProblems(plan: WritePlanDto | undefined): DiagnosticDto[] {
  return (plan?.diagnostics ?? []).filter(
    (d) =>
      d.field?.startsWith('/assets') ||
      d.field?.startsWith('/sounds') ||
      d.code === 'design.sound-cast-replaced',
  );
}
