import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { ICON_NAMES, Icon } from './Icon';

describe('Icon', () => {
  it('is hidden from assistive tech without a label', () => {
    const { container } = render(<Icon name="folder" />);
    expect(container.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
  });

  it('exposes a label as an image role', () => {
    render(<Icon name="warning" label="Warning" />);
    expect(screen.getByRole('img', { name: 'Warning' })).toBeTruthy();
  });

  it('ships every required glyph', () => {
    for (const name of [
      'folder',
      'file',
      'xml',
      'image',
      'sound',
      'patch',
      'warning',
      'check',
      'plus',
      'trash',
      'copy',
      'play',
      'chevron',
      'search',
      'link',
      'lock',
      'refresh',
      'settings',
    ]) {
      expect(ICON_NAMES).toContain(name);
    }
  });

  it('rotates by quarter turns', () => {
    const { container } = render(<Icon name="chevron" turn={1} />);
    expect(container.querySelector('svg')?.getAttribute('class')).toContain('rotate-90');
  });
});
