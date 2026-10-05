import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { CePlatformFields } from './CePlatformFields';

const BASE = { oneHanded: false, beltFed: false };

describe('CePlatformFields', () => {
  it('turns the platform flag on and off', () => {
    const onChange = vi.fn();
    const { rerender } = render(<CePlatformFields block={BASE} onChange={onChange} />);
    fireEvent.click(screen.getByRole('switch', { name: /platform/i }));
    expect(onChange).toHaveBeenLastCalledWith({ isWeaponPlatform: true });
    rerender(<CePlatformFields block={{ ...BASE, isWeaponPlatform: true }} onChange={onChange} />);
    fireEvent.click(screen.getByRole('switch', { name: /platform/i }));
    expect(onChange).toHaveBeenLastCalledWith({ isWeaponPlatform: false });
  });

  it('adds an attachment link and a default part to the lists', () => {
    const onChange = vi.fn();
    render(
      <CePlatformFields
        block={{ ...BASE, attachmentLinks: [{ attachment: 'A' }] }}
        onChange={onChange}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Add an attachment' }));
    expect(onChange).toHaveBeenLastCalledWith({
      attachmentLinks: [{ attachment: 'A' }, { attachment: '' }],
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add a default part' }));
    expect(onChange).toHaveBeenLastCalledWith({ defaultGraphicParts: [{}] });
  });

  it('summarises the counts when the weapon is a platform', () => {
    render(
      <CePlatformFields
        block={{ ...BASE, isWeaponPlatform: true, attachmentLinks: [{ attachment: 'A' }] }}
        onChange={() => undefined}
      />,
    );
    expect(screen.getByText('1 attachment, 0 parts')).toBeTruthy();
  });
});
