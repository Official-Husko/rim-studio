import { describe, expect, it } from 'vitest';
import { purposeOf } from './purposes';

describe('purposeOf', () => {
  it('knows the standard entries', () => {
    expect(purposeOf('About/About.xml')?.key).toBe('project.create.purpose.aboutXml');
    expect(purposeOf('Defs/ThingDefs_Misc/Weapons')?.key).toBe('project.create.purpose.weapons');
    expect(purposeOf('.gitignore')?.key).toBe('project.create.purpose.gitignore');
  });

  it('names a version folder and looks inside it', () => {
    expect(purposeOf('1.6')).toEqual({
      key: 'project.create.purpose.versionFolder',
      version: '1.6',
    });
    expect(purposeOf('1.6/Compat/CombatExtended')?.key).toBe('project.create.purpose.ce');
  });

  it('knows placeholder files and says nothing about unknown paths', () => {
    expect(purposeOf('Patches/.gitkeep')?.key).toBe('project.create.purpose.gitkeep');
    expect(purposeOf('Mystery')).toBeUndefined();
  });
});
