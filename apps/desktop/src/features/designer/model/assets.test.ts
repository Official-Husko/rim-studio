import type { DraftDto, WritePlanDto } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import { fixture, importsDraft } from '../testSupport';
import {
  assetsWith,
  baseName,
  clipCopies,
  copyOfSource,
  importProblems,
  plannedSoundName,
  rangeOf,
  shortHash,
  shotWith,
  soundsWith,
  texPathOf,
  textureOrigin,
} from './assets';

const plan = () => fixture<WritePlanDto>('designer-assets-plan-imports');
const spec = () => importsDraft().spec;

describe('assets model', () => {
  it('sets and clears a texture slot and drops the member when nothing is left', () => {
    const s = spec();
    expect(assetsWith(s, 'texture', '/a/b.png')).toMatchObject({ texture: '/a/b.png' });
    const only = { ...s, assets: { texture: '/a/b.png' } };
    expect(assetsWith(only, 'texture', undefined)).toBeUndefined();
    expect(assetsWith(s, 'texture', undefined)).toEqual({
      projectileTexture: s.assets?.projectileTexture,
    });
  });

  it('changes one member of the shot and removes an empty clip list', () => {
    const shot = spec().sounds?.shot;
    expect(shotWith(shot, 'maxSimultaneous', 5).maxSimultaneous).toBe(5);
    expect(shotWith(shot, 'clips', []).clips).toBeUndefined();
    expect(shotWith(shot, 'volume', undefined).volume).toBeUndefined();
    expect(shotWith(undefined, 'clips', ['/x.wav'])).toEqual({ clips: ['/x.wav'] });
  });

  it('removes the sounds member when the shot goes and nothing else is there', () => {
    expect(soundsWith(spec(), undefined)).toBeUndefined();
    expect(soundsWith(spec(), { clips: ['/x.wav'] })).toEqual({ shot: { clips: ['/x.wav'] } });
  });

  it('builds a range from two ends only', () => {
    expect(rangeOf(1, 2)).toEqual({ min: 1, max: 2 });
    expect(rangeOf(undefined, undefined)).toBeUndefined();
    expect(rangeOf(1, undefined)).toBe('incomplete');
  });

  it('finds where a texture comes from', () => {
    const s = spec();
    expect(textureOrigin(s, 'texture', plan())).toEqual({
      kind: 'imported',
      target: 'Things/Item/Equipment/WeaponRanged/DM_Carbine',
    });
    expect(textureOrigin(s, 'texture', undefined)).toEqual({ kind: 'imported' });
    const clone = fixture<DraftDto>('designer-fields-draft-rifle-own-edited').spec;
    expect(textureOrigin(clone, 'texture', undefined)).toEqual({
      kind: 'shared',
      texPath: 'Things/Item/Equipment/WeaponRanged/BoltActionRifle',
    });
    expect(textureOrigin({ ...clone, texturePath: undefined }, 'texture', undefined)).toEqual({
      kind: 'reserved',
    });
  });

  it('reads the planned copies, the target texture path and the sound definition name', () => {
    const p = plan();
    expect(clipCopies(p)).toHaveLength(2);
    expect(copyOfSource(p, spec().assets?.texture ?? '')?.path).toContain('Textures/');
    expect(texPathOf('Textures/Things/Projectile/Bullet_DM_Carbine.png')).toBe(
      'Things/Projectile/Bullet_DM_Carbine',
    );
    expect(plannedSoundName(p)).toBe('DM_Carbine_Shot');
    expect(plannedSoundName(undefined)).toBeUndefined();
  });

  it('picks the import problems out of a plan', () => {
    const codes = importProblems(plan()).map((d) => d.code);
    expect(codes).toContain('design.sound-stereo');
    expect(codes).toContain('design.sound-cast-replaced');
    expect(codes).not.toContain('design.duplicate-capacity');
  });

  it('shortens hashes and names files', () => {
    expect(shortHash('0123456789abcdef')).toBe('0123456789ab');
    expect(baseName('C:\\Art\\gun.png')).toBe('gun.png');
    expect(baseName('/a/b/gun.png')).toBe('gun.png');
  });
});
