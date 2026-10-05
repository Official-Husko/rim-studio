import { describe, expect, it } from 'vitest';
import { parseAnswer, readDevLinks } from './dev-links';

describe('dev links', () => {
  it('reads nothing from a plain route', () => {
    const links = readDevLinks('#/weapons');
    expect(links.project).toBeUndefined();
    expect(links.set).toEqual([]);
  });

  it('reads the project, a clone, typed numbers, a tool and quiz answers', () => {
    const links = readDevLinks(
      '#/weapons?project=/tmp/Mod&clone=Gun_A:My_A&set=/ranged/damage=22&tool=blade:Cut+Stab&quiz=1&qa=tier:1,weaker,typed:12&def=Gun_A',
    );
    expect(links.project).toBe('/tmp/Mod');
    expect(links.clone).toEqual({ source: 'Gun_A', name: 'My_A' });
    expect(links.set).toEqual([['/ranged/damage', 22]]);
    expect(links.tools).toEqual([{ label: 'blade', capacities: ['Cut', 'Stab'] }]);
    expect(links.answers).toEqual([
      { kind: 'tier', tier: 1 },
      { kind: 'weaker' },
      { kind: 'typed', value: 12 },
    ]);
    expect(links.def).toBe('Gun_A');
  });

  it('reads the shared projectile and expand switches', () => {
    const links = readDevLinks('#/weapons?clone=Gun_A:My_A&shared=1&expand=1');
    expect(links.shared).toBe(true);
    expect(links.expand).toBe(true);
    expect(readDevLinks('#/weapons').shared).toBe(false);
  });

  it('ignores an unknown answer', () => {
    expect(parseAnswer('whatever')).toBeUndefined();
    expect(parseAnswer('role:blade')).toEqual({ kind: 'role', role: 'blade' });
  });
});
