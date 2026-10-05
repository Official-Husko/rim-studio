import { afterEach, describe, expect, it } from 'vitest';
import { navigate, parseHash, route, startRouter } from './route';

afterEach(() => {
  location.hash = '';
  route.value = 'setup';
});

describe('parseHash', () => {
  it.each([
    ['#/weapons', 'weapons'],
    ['#/patches?x=1', 'patches'],
    ['#weapons', 'weapons'],
    ['', 'setup'],
    ['#/nope', 'setup'],
    ['#/gallery/buttons', 'gallery'],
  ])('%s gives %s', (hash, expected) => {
    expect(parseHash(hash)).toBe(expected);
  });
});

describe('route signal', () => {
  it('navigate writes the hash and the signal', () => {
    navigate('project');
    expect(location.hash).toBe('#/project');
    expect(route.value).toBe('project');
  });

  it('follows hashchange events', async () => {
    const stop = startRouter();
    location.hash = '/patches';
    await new Promise((r) => setTimeout(r, 20));
    expect(route.value).toBe('patches');
    stop();
  });
});
