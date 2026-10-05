import type { ProjectCreateRequest } from 'rimstudio-ipc-types';
import { splitVersions } from './model';

/** What the new mod form holds. */
export interface NewModForm {
  name: string;
  author: string;
  packageId: string;
  /** Typed as "1.6" or "1.5, 1.6". */
  versions: string;
  description: string;
  /** The folder name below the chosen parent folder. */
  folderName: string;
  versionedFolders: boolean;
  patchesFolder: boolean;
  texturesFolder: boolean;
  soundsFolder: boolean;
  languagesFolder: boolean;
  assembliesFolder: boolean;
  cePatchFolder: boolean;
  sourceFolder: boolean;
  gitignore: boolean;
  ignoreSourceArt: boolean;
  readme: boolean;
  credits: boolean;
  placeholderFiles: boolean;
}

/** The form of a fresh dialog: the defaults of the documented scaffold. */
export function emptyForm(): NewModForm {
  return {
    name: '',
    author: '',
    packageId: '',
    versions: '1.6',
    description: '',
    folderName: '',
    versionedFolders: false,
    patchesFolder: true,
    texturesFolder: true,
    soundsFolder: true,
    languagesFolder: false,
    assembliesFolder: false,
    cePatchFolder: false,
    sourceFolder: false,
    gitignore: false,
    ignoreSourceArt: false,
    readme: false,
    credits: false,
    placeholderFiles: false,
  };
}

/** The create request of a form and the full path of the new mod folder. */
export function requestOf(form: NewModForm, path: string): ProjectCreateRequest {
  return {
    path,
    name: form.name.trim(),
    packageId: form.packageId.trim(),
    author: form.author.trim(),
    description: form.description.trim(),
    supportedVersions: splitVersions(form.versions),
    versionedFolders: form.versionedFolders,
    patchesFolder: form.patchesFolder,
    languagesFolder: form.languagesFolder,
    assembliesFolder: form.assembliesFolder,
    cePatchFolder: form.cePatchFolder,
    placeholderFiles: form.placeholderFiles,
    texturesFolder: form.texturesFolder,
    soundsFolder: form.soundsFolder,
    sourceFolder: form.sourceFolder,
    gitignore: form.gitignore,
    ignoreSourceArt: form.ignoreSourceArt && form.sourceFolder,
    readme: form.readme,
    credits: form.credits,
  };
}

/** One entry of the preview. */
export interface PreviewEntry {
  path: string;
  kind: 'folder' | 'file';
}

const WEAPONS = 'Defs/ThingDefs_Misc/Weapons';
const TEXTURES = 'Textures/Things';

/** Collects entries; every folder above an entry is added as well. */
class Collector {
  readonly entries = new Map<string, PreviewEntry>();

  private parents(path: string): void {
    const parts = path.split('/');
    for (let i = 1; i < parts.length; i += 1) {
      const folder = parts.slice(0, i).join('/');
      if (!this.entries.has(folder)) this.entries.set(folder, { path: folder, kind: 'folder' });
    }
  }

  folder(path: string, placeholder: boolean): void {
    this.parents(path);
    this.entries.set(path, { path, kind: 'folder' });
    if (placeholder) this.file(`${path}/.gitkeep`);
  }

  file(path: string): void {
    this.parents(path);
    this.entries.set(path, { path, kind: 'file' });
  }
}

/**
 * The paths that creating the mod will write, as the scaffold of docs/features/mod-layout.md
 * section 9 lays them out. A preview for the eye only: the backend writes the real files, and a
 * test keeps this list equal to the trees the backend produces.
 */
export function scaffoldPreview(form: NewModForm): PreviewEntry[] {
  const out = new Collector();
  const versions = splitVersions(form.versions);
  const roots = form.versionedFolders ? (versions.length > 0 ? versions : ['1.6']) : [''];
  const keep = form.placeholderFiles;

  out.file('About/About.xml');
  for (const root of roots) {
    const at = (path: string): string => (root ? `${root}/${path}` : path);
    const leaves = [WEAPONS, 'Defs/SoundDefs'];
    if (form.patchesFolder) leaves.push('Patches');
    if (form.texturesFolder)
      leaves.push(
        `${TEXTURES}/Item/Equipment/WeaponRanged`,
        `${TEXTURES}/Item/Equipment/WeaponMelee`,
        `${TEXTURES}/Projectile`,
      );
    if (form.soundsFolder) leaves.push('Sounds/Weapons');
    if (form.languagesFolder) leaves.push('Languages/English/Keyed');
    if (form.assembliesFolder) leaves.push('Assemblies');
    if (form.cePatchFolder) leaves.push('Compat/CombatExtended/Patches');
    for (const leaf of leaves) out.folder(at(leaf), keep);
  }
  if (form.versionedFolders) out.folder('Common', keep);
  if (form.versionedFolders || form.cePatchFolder) out.file('LoadFolders.xml');
  if (form.sourceFolder) out.folder('Source/Art', keep);
  if (form.gitignore) out.file('.gitignore');
  if (form.readme) out.file('README.md');
  if (form.credits) out.file('Credits.txt');
  return [...out.entries.values()].sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
}
