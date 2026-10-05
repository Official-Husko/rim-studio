import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { installTransport } from '../testSupport';
import { ChipListEditor } from './ChipListEditor';

describe('ChipListEditor', () => {
  it('adds an entry on Enter, ignoring a repeat in other letter case', () => {
    const onChange = vi.fn();
    renderWithProviders(<ChipListEditor label="Load after" items={['A.B']} onChange={onChange} />);
    const field = screen.getByRole('textbox', { name: 'Load after' });
    fireEvent.input(field, { target: { value: 'a.b' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.input(field, { target: { value: 'c.d' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add to Load after' }));
    expect(onChange).toHaveBeenCalledWith(['A.B', 'c.d']);
  });

  it('removes a chip', () => {
    const onChange = vi.fn();
    renderWithProviders(
      <ChipListEditor label="Load after" items={['A.B', 'C.D']} onChange={onChange} />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Remove A.B' }));
    expect(onChange).toHaveBeenCalledWith(['C.D']);
  });

  it('fills the list from the library search', async () => {
    installTransport();
    const onChange = vi.fn();
    renderWithProviders(
      <ChipListEditor label="Load after" items={[]} onChange={onChange} library />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Pick from library: Load after' }));
    fireEvent.input(screen.getByRole('searchbox', { name: 'Search the library for Load after' }), {
      target: { value: 'combat' },
    });
    fireEvent.click(await screen.findByRole('button', { name: 'Add Combat Extended' }));
    await waitFor(() => expect(onChange).toHaveBeenCalledWith(['CETeam.CombatExtended']));
  });

  it('cannot be changed when disabled', () => {
    renderWithProviders(
      <ChipListEditor label="X" items={['a']} onChange={() => undefined} disabled />,
    );
    expect(screen.queryByRole('button', { name: 'Remove a' })).toBeNull();
    expect(screen.getByRole<HTMLInputElement>('textbox', { name: 'X' }).disabled).toBe(true);
  });
});
