import { describe, expect, it } from 'vitest';
import { probeFeatures } from './probe';

const all = { css: { supports: () => true }, hasLayerRule: true, hasPropertyRule: true };

describe('probeFeatures', () => {
  it('reports nothing for a modern engine', () => {
    expect(probeFeatures(all)).toEqual([]);
  });

  it('names each missing feature', () => {
    const old = {
      css: { supports: (c: string) => !c.includes('color-mix') && !c.includes(':has') },
      hasLayerRule: false,
      hasPropertyRule: true,
    };
    expect(probeFeatures(old)).toEqual(['@layer', 'color-mix()', ':has()']);
  });

  it('treats a missing CSS object as lacking everything it tests', () => {
    expect(probeFeatures({ hasLayerRule: true, hasPropertyRule: true })).toEqual([
      'color-mix()',
      'container queries',
      ':has()',
    ]);
  });

  it('survives a supports() that throws', () => {
    const throwing = {
      css: {
        supports: () => {
          throw new Error('x');
        },
      },
      hasLayerRule: true,
      hasPropertyRule: true,
    };
    expect(probeFeatures(throwing)).toHaveLength(3);
  });
});
