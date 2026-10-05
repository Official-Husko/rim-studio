import { afterEach, describe, expect, it } from 'vitest';
import { applyTheme, density, setDensity } from './theme';

afterEach(() => setDensity('comfortable'));

describe('theme', () => {
  it('applies dark and the density to the root element', () => {
    setDensity('compact');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    expect(document.documentElement.getAttribute('data-density')).toBe('compact');
    expect(density.value).toBe('compact');
  });

  it('applyTheme sets the attributes on a given element', () => {
    const el = document.createElement('div');
    applyTheme(el);
    expect(el.getAttribute('data-theme')).toBe('dark');
  });
});
