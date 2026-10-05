import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { DesignerCloneResponse } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import { CloneNotes } from './CloneNotes';

describe('CloneNotes', () => {
  it('shows the notes of a real clone and can be dismissed', () => {
    const notes = fixture<DesignerCloneResponse>('designer-fields-clone-zeushammer').notes ?? [];
    const onDismiss = vi.fn();
    renderWithProviders(<CloneNotes notes={notes} onDismiss={onDismiss} />);
    expect(screen.getByText(notes[0] ?? '')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss the clone notes' }));
    expect(onDismiss).toHaveBeenCalled();
  });

  it('shows nothing without notes', () => {
    const { container } = renderWithProviders(
      <CloneNotes notes={[]} onDismiss={() => undefined} />,
    );
    expect(container.textContent).toBe('');
  });
});
