import { describe, expect, it } from 'vitest';
import { actionText, kindText, ratingText, ratingTone, sourceText } from './labels';

describe('output labels', () => {
  it('names every action and role', () => {
    expect(actionText('create')).toBe('New');
    expect(actionText('update-region')).toBe('Update');
    expect(actionText('unchanged')).toBe('Unchanged');
    expect(kindText('vanilla-defs')).toBe('Weapon definitions');
    expect(kindText('ce-patch')).toBe('Combat Extended patch');
    expect(kindText('load-folders')).toBe('Load folders');
    expect(kindText('about')).toBe('About');
  });

  it('words a rating and keeps colour from being the only signal', () => {
    expect(ratingText('reliable')).toBe('Reliable');
    expect(ratingText('unmeasured')).toBe('Unmeasured');
    expect(ratingTone('rough')).toBe('warning');
    expect(ratingTone('unreliable')).toBe('danger');
  });

  it('says where a number came from', () => {
    expect(sourceText({ kind: 'predicted', predictor: 'median', n: 4 })).toBe(
      'predicted from 4 converted weapons',
    );
    expect(sourceText({ kind: 'identity', n: 18 })).toBe(
      'same as vanilla, from 18 converted weapons',
    );
    expect(sourceText({ kind: 'first-of-set' })).toBe('first of the ammo set');
    expect(sourceText({ kind: 'vanilla' })).toBe('vanilla value');
    expect(sourceText({ kind: 'typed' })).toBe('typed by you');
  });
});
