import { describe, expect, it } from 'vitest';
import { createNameState } from './wizard-name';
import { readPrefix } from './wizard-prefs';

describe('name state', () => {
  it('follows the label until the user types a def name, and again once it is cleared', () => {
    const name = createNameState();
    name.setPrefix('RS_');
    name.setLabel('Long Rifle');
    expect(name.defName.value).toBe('RS_LongRifle');
    name.setDefName('RS_Mine');
    name.setLabel('Other');
    expect(name.defName.value).toBe('RS_Mine');
    name.setDefName('');
    expect(name.defName.value).toBe('RS_Other');
  });

  it('remembers the prefix and forgets the label on reset', () => {
    const name = createNameState();
    name.setPrefix('QA_');
    expect(readPrefix()).toBe('QA_');
    name.setLabel('Gun');
    name.reset();
    expect(name.label.value).toBe('');
    expect(name.defName.value).toBe('');
    expect(name.prefix.value).toBe('QA_');
  });
});
