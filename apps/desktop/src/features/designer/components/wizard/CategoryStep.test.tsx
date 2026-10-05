import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { CategoryStep } from './CategoryStep';
import { catalogFixture } from './wizardTestSupport';

describe('CategoryStep', () => {
  it('shows the families of guns and of melee weapons, and the types of the rifle family first', () => {
    renderWithProviders(
      <CategoryStep
        catalog={catalogFixture()}
        error={undefined}
        chosen={undefined}
        onChoose={() => undefined}
      />,
    );
    expect(screen.getByRole('region', { name: 'Guns and bows' })).toBeTruthy();
    expect(screen.getByRole('region', { name: 'Melee' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Sniper rifle' })).toBeTruthy();
    expect(screen.getByText(/longest reach and hardest hit/)).toBeTruthy();
  });

  it('opens another family and chooses a type', () => {
    const onChoose = vi.fn();
    renderWithProviders(
      <CategoryStep
        catalog={catalogFixture()}
        error={undefined}
        chosen={undefined}
        onChoose={onChoose}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Submachine gun' }));
    fireEvent.click(screen.getByRole('button', { name: 'Light SMG' }));
    expect(onChoose).toHaveBeenCalledWith('smg/light');
  });

  it('says which type is chosen', () => {
    renderWithProviders(
      <CategoryStep
        catalog={catalogFixture()}
        error={undefined}
        chosen="rifle/bolt"
        onChoose={() => undefined}
      />,
    );
    expect(
      screen
        .getByRole('button', { name: 'Bolt action hunting rifle' })
        .getAttribute('aria-pressed'),
    ).toBe('true');
    expect(screen.getByRole('status').textContent).toContain('Bolt action hunting rifle');
  });

  it('shows a loading state, an error and the missing install notice', () => {
    const { unmount } = renderWithProviders(
      <CategoryStep
        catalog={undefined}
        error={undefined}
        chosen={undefined}
        onChoose={() => undefined}
      />,
    );
    expect(screen.getAllByText('Loading the weapon types').length).toBeGreaterThan(0);
    unmount();
    const err = renderWithProviders(
      <CategoryStep
        catalog={undefined}
        error={{ code: 'x.fail', message: 'broken', errorId: 'e' }}
        chosen={undefined}
        onChoose={() => undefined}
      />,
    );
    expect(screen.getByText('broken')).toBeTruthy();
    err.unmount();
    renderWithProviders(
      <CategoryStep
        catalog={{ ...catalogFixture(), proposalsAvailable: false }}
        error={undefined}
        chosen={undefined}
        onChoose={() => undefined}
      />,
    );
    expect(screen.getByText('No game install loaded')).toBeTruthy();
  });
});
