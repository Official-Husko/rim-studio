import type { ProjectCreateRequest } from 'rimstudio-ipc-types';

/** What the new mod form holds. */
export interface NewModForm {
  name: string;
  author: string;
  packageId: string;
  /** The supported game versions that are ticked, as "1.6". */
  versions: string[];
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
export function emptyForm(version = '1.6'): NewModForm {
  return {
    name: '',
    author: '',
    packageId: '',
    versions: [version],
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
    supportedVersions: form.versions,
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
