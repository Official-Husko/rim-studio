import type { MessageKey } from '~/shared/i18n';

// What each folder and file of the recommended structure is for, in plain words. A display lookup by
// path: the backend decides which entries exist, this only says what they are for.

const VERSION_FOLDER = /^\d+\.\d+$/;

const BY_PATH: Record<string, MessageKey> = {
  About: 'project.create.purpose.about',
  'About/About.xml': 'project.create.purpose.aboutXml',
  'LoadFolders.xml': 'project.create.purpose.loadFolders',
  Common: 'project.create.purpose.common',
  Defs: 'project.create.purpose.defs',
  'Defs/ThingDefs_Misc': 'project.create.purpose.thingDefs',
  'Defs/ThingDefs_Misc/Weapons': 'project.create.purpose.weapons',
  'Defs/SoundDefs': 'project.create.purpose.soundDefs',
  Patches: 'project.create.purpose.patches',
  Compat: 'project.create.purpose.compat',
  'Compat/CombatExtended': 'project.create.purpose.ce',
  'Compat/CombatExtended/Patches': 'project.create.purpose.cePatches',
  Textures: 'project.create.purpose.textures',
  'Textures/Things': 'project.create.purpose.texturesThings',
  'Textures/Things/Item': 'project.create.purpose.texturesItem',
  'Textures/Things/Item/Equipment': 'project.create.purpose.texturesEquipment',
  'Textures/Things/Item/Equipment/WeaponRanged': 'project.create.purpose.texturesRanged',
  'Textures/Things/Item/Equipment/WeaponMelee': 'project.create.purpose.texturesMelee',
  'Textures/Things/Projectile': 'project.create.purpose.texturesProjectile',
  Sounds: 'project.create.purpose.sounds',
  'Sounds/Weapons': 'project.create.purpose.soundsWeapons',
  Languages: 'project.create.purpose.languages',
  'Languages/English': 'project.create.purpose.languagesEnglish',
  'Languages/English/Keyed': 'project.create.purpose.keyed',
  Assemblies: 'project.create.purpose.assemblies',
  Source: 'project.create.purpose.source',
  'Source/Art': 'project.create.purpose.sourceArt',
  '.gitignore': 'project.create.purpose.gitignore',
  'README.md': 'project.create.purpose.readme',
  'Credits.txt': 'project.create.purpose.credits',
};

/** The purpose of an entry, with the version folder name when the entry is a version folder. */
export function purposeOf(path: string): { key: MessageKey; version?: string } | undefined {
  const parts = path.split('/');
  const first = parts[0] ?? '';
  if (VERSION_FOLDER.test(first)) {
    if (parts.length === 1) return { key: 'project.create.purpose.versionFolder', version: first };
    const inside = BY_PATH[parts.slice(1).join('/')];
    return inside ? { key: inside } : undefined;
  }
  const key = BY_PATH[path];
  if (key) return { key };
  if (path.endsWith('/.gitkeep') || path === '.gitkeep') {
    return { key: 'project.create.purpose.gitkeep' };
  }
  return undefined;
}
