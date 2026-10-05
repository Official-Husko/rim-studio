import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { DesignerStructureDefaultsResponse } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import { StructureBanner } from './StructureBanner';

describe('StructureBanner', () => {
  it('offers the structure of the nearest reference weapon', () => {
    const suggestion = fixture<DesignerStructureDefaultsResponse>('designer_structure_defaults');
    const onApply = vi.fn();
    renderWithProviders(<StructureBanner suggestion={suggestion} onApply={onApply} />);
    expect(screen.getByText(/Nearest reference weapon/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Use this structure' }));
    expect(onApply).toHaveBeenCalled();
  });

  it('stays hidden when nothing would be filled', () => {
    const suggestion = fixture<DesignerStructureDefaultsResponse>('designer_structure_defaults');
    const { container } = renderWithProviders(
      <StructureBanner suggestion={{ ...suggestion, filled: [] }} onApply={() => undefined} />,
    );
    expect(container.textContent).toBe('');
  });
});
