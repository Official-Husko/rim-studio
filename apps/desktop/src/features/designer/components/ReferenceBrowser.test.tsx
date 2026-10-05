import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { createReferenceStore } from '../reference-store';
import { installTransport } from '../testSupport';
import { ReferenceBrowser } from './ReferenceBrowser';

async function setup(canClone = true, canAnchor = true) {
  const transport = installTransport();
  const store = createReferenceStore();
  await store.load();
  const handlers = { onClone: vi.fn(), onAnchor: vi.fn(), onShowDefinition: vi.fn() };
  renderWithProviders(
    <ReferenceBrowser store={store} canClone={canClone} canAnchor={canAnchor} {...handlers} />,
  );
  return { transport, store, ...handlers };
}

describe('ReferenceBrowser', () => {
  it('lists the real vanilla weapons weakest first with their stats', async () => {
    await setup();
    const cards = screen.getAllByRole('listitem');
    expect(cards.length).toBe(20);
    expect(within(cards[0] as HTMLElement).getByText('flamebow')).toBeTruthy();
    expect(screen.getByText('20 of 20 weapons')).toBeTruthy();
  });

  it('flips the order to strongest first', async () => {
    await setup();
    fireEvent.click(screen.getByRole('button', { name: 'Strongest first' }));
    expect(
      within(screen.getAllByRole('listitem')[0] as HTMLElement).getByText('minigun'),
    ).toBeTruthy();
  });

  it('filters by the text typed', async () => {
    await setup();
    fireEvent.input(screen.getByRole('searchbox', { name: 'Search by name' }), {
      target: { value: 'rifle' },
    });
    await waitFor(() => expect(screen.getAllByRole('listitem').length).toBe(5));
  });

  it('clones, anchors and shows the real definition of a weapon', async () => {
    const h = await setup();
    const card = screen
      .getAllByRole('listitem')
      .find((li) => li.textContent?.includes('Gun_BoltActionRifle')) as HTMLElement;
    fireEvent.click(within(card).getByRole('button', { name: 'Clone' }));
    fireEvent.click(within(card).getByRole('button', { name: 'Use as anchor' }));
    fireEvent.click(within(card).getByRole('button', { name: 'Real definition' }));
    expect(h.onClone.mock.calls[0]?.[0].defName).toBe('Gun_BoltActionRifle');
    expect(h.onAnchor).toHaveBeenCalled();
    expect(h.onShowDefinition).toHaveBeenCalled();
  });

  it('disables clone without a project and anchor without a draft', async () => {
    await setup(false, false);
    const card = screen.getAllByRole('listitem')[0] as HTMLElement;
    expect(
      (within(card).getByRole('button', { name: 'Clone' }) as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(
      (within(card).getByRole('button', { name: 'Use as anchor' }) as HTMLButtonElement).disabled,
    ).toBe(true);
  });

  it('switches to melee weapons', async () => {
    const h = await setup();
    fireEvent.click(screen.getByRole('radio', { name: 'Melee' }));
    await waitFor(() => expect(h.transport.calls.at(-1)?.request).toMatchObject({ kind: 'melee' }));
  });
});
