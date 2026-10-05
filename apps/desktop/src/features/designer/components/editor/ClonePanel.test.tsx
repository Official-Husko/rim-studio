import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { DesignerCloneDiffResponse } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import { ClonePanel } from './ClonePanel';

describe('ClonePanel', () => {
  it('shows the changed field and what it does to the readouts', () => {
    const diff = fixture<DesignerCloneDiffResponse>('designer_clone_diff');
    renderWithProviders(<ClonePanel diff={diff} />);
    expect(screen.getByText('Changes from bolt-action rifle')).toBeTruthy();
    expect(screen.getByText('ranged damage')).toBeTruthy();
    expect(screen.getByText('+1.25')).toBeTruthy();
    expect(screen.getByText(/shares with Gun_BoltActionRifle/)).toBeTruthy();
  });

  it('says nothing differs yet', () => {
    const diff = fixture<DesignerCloneDiffResponse>('designer_clone_diff');
    renderWithProviders(<ClonePanel diff={{ ...diff, changes: [], readouts: [], notes: [] }} />);
    expect(screen.getByText('Nothing differs from the source yet.')).toBeTruthy();
  });

  it('renders nothing for a draft that is not a clone', () => {
    const { container } = renderWithProviders(<ClonePanel diff={undefined} />);
    expect(container.textContent).toBe('');
  });
});
