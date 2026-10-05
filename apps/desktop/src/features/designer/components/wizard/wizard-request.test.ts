import { describe, expect, it } from 'vitest';
import { buildRequest } from './wizard-request';
import { defaultDescriptors, emptyChoice, findArchetype } from './wizard-model';
import { catalogFixture } from './wizardTestSupport';

describe('buildRequest', () => {
  const catalog = catalogFixture();
  const sniper = findArchetype(catalog, 'rifle/sniper');
  const sword = findArchetype(catalog, 'sword/long');

  it('has nothing to ask without a type', () => {
    expect(buildRequest(undefined, emptyChoice(), undefined)).toBeUndefined();
  });

  it('sends the defaults of a gun and the balance target', () => {
    if (!sniper) throw new Error('missing');
    const choice = {
      ...emptyChoice(),
      archetypeId: sniper.id,
      descriptors: defaultDescriptors(sniper),
    };
    expect(buildRequest(sniper, choice, undefined)).toEqual({
      kind: 'ranged',
      archetype: 'rifle/sniper',
      descriptors: {
        action: 'bolt',
        rof: { class: 'medium' },
        calibre: 'huge',
        handling: 'standard',
      },
      balanceTarget: 'typical',
      mode: 'vanilla',
    });
  });

  it('leaves out what a melee weapon does not take', () => {
    if (!sword) throw new Error('missing');
    const choice = {
      ...emptyChoice(),
      descriptors: { ...defaultDescriptors(sword), action: 'bolt', calibre: 'large' },
    };
    expect(buildRequest(sword, choice, undefined)?.descriptors).toEqual({
      rof: { class: 'medium' },
      handling: 'standard',
    });
  });

  it('sends the ammo set instead of the class in Combat Extended mode, and the draft of a retune', () => {
    if (!sniper) throw new Error('missing');
    const choice = {
      ...emptyChoice(),
      ceCalibre: true,
      descriptors: { ...defaultDescriptors(sniper), ammoSet: 'AmmoSet_338Norma' },
    };
    const draft = { schemaVersion: 2, kind: 'ranged', calibration: 'simple', spec: {} } as never;
    const request = buildRequest(sniper, choice, draft);
    expect(request?.mode).toBe('combat-extended');
    expect(request?.descriptors.ammoSet).toBe('AmmoSet_338Norma');
    expect(request?.descriptors.calibre).toBeUndefined();
    expect(request?.draft).toBe(draft);
  });
});
