import { screen, waitFor } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { fixture, installTransport } from '../testSupport';
import { DefinitionDialog } from './DefinitionDialog';

describe('DefinitionDialog', () => {
  it('shows the resolved definition of the bolt-action rifle as XML', async () => {
    installTransport({ defs_get_resolved: () => fixture('defs-resolved-bolt-action-rifle') });
    renderWithProviders(
      <DefinitionDialog defName="Gun_BoltActionRifle" onClose={() => undefined} />,
    );
    await waitFor(() => expect(screen.getByLabelText('XML of Gun_BoltActionRifle')).toBeTruthy());
    expect(screen.getByLabelText('XML of Gun_BoltActionRifle').textContent).toContain(
      '<WorkToMake>12000</WorkToMake>',
    );
    expect(screen.getByText(/Ludeon.RimWorld/)).toBeTruthy();
  });

  it('shows the error when the definition cannot be read', async () => {
    installTransport({
      defs_get_resolved: () => {
        throw { code: 'defs.not-found', message: 'no such def', errorId: 'e-1' };
      },
    });
    renderWithProviders(<DefinitionDialog defName="Nope" onClose={() => undefined} />);
    await waitFor(() => expect(screen.getByText('no such def')).toBeTruthy());
  });

  it('stays closed without a def name', () => {
    installTransport();
    renderWithProviders(<DefinitionDialog defName={undefined} onClose={() => undefined} />);
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
