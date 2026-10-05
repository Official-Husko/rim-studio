import { afterEach, describe, expect, it } from 'vitest';
import { devLink } from './devLinks';

afterEach(() => {
  window.location.hash = '';
});

describe('devLink', () => {
  it('reads a parameter of the hash query', () => {
    window.location.hash = '#/project?open=%2Fmods%2FX&tab=layout';
    expect(devLink('open')).toBe('/mods/X');
    expect(devLink('tab')).toBe('layout');
    expect(devLink('file')).toBeUndefined();
  });

  it('has nothing without a query', () => {
    window.location.hash = '#/project';
    expect(devLink('open')).toBeUndefined();
  });
});
