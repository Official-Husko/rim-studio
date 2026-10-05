import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { ArchetypeBar } from './ArchetypeBar';

describe('ArchetypeBar', () => {
  it('names the type and offers to propose again', () => {
    const onRetune = vi.fn();
    renderWithProviders(
      <ArchetypeBar
        choice={{ archetype: 'rifle/sniper', descriptors: {}, balance: 'typical', mode: 'vanilla' }}
        onRetune={onRetune}
      />,
    );
    expect(screen.getByText(/proposed for rifle\/sniper/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Propose again' }));
    expect(onRetune).toHaveBeenCalled();
  });
});
