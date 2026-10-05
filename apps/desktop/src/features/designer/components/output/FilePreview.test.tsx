import { fireEvent, screen } from '@testing-library/preact';
import type { WritePlanDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { fixture } from '../../testSupport';
import { FilePreview } from './FilePreview';

describe('FilePreview', () => {
  it('shows the XML of a new file as it will be written', () => {
    const file = fixture<WritePlanDto>('designer-output-plan-vanilla').files[0];
    if (!file) throw new Error('fixture');
    renderWithProviders(<FilePreview file={file} />);
    expect(screen.getByRole('region', { name: 'XML of Gun_OutRifle.xml' })).toBeTruthy();
    expect(screen.getByText('As it will be written')).toBeTruthy();
    expect(screen.queryByRole('radio', { name: 'Changes' })).toBeNull();
  });

  it('shows the changes of an update and lets the user read the whole file', () => {
    const file = fixture<WritePlanDto>('designer-output-plan-ce-update').files[0];
    if (!file) throw new Error('fixture');
    renderWithProviders(<FilePreview file={file} />);
    expect(screen.getByRole('region', { name: 'Changes to Gun_OutRifle.xml' })).toBeTruthy();
    expect(screen.getByText('21', { exact: false })).toBeTruthy();
    fireEvent.click(screen.getByRole('radio', { name: 'Whole file' }));
    expect(screen.getByRole('region', { name: 'XML of Gun_OutRifle.xml' })).toBeTruthy();
  });

  it('opens the same text wide in a dialog', () => {
    const file = fixture<WritePlanDto>('designer-output-plan-vanilla').files[0];
    if (!file) throw new Error('fixture');
    renderWithProviders(<FilePreview file={file} />);
    fireEvent.click(screen.getByRole('button', { name: 'Open wide' }));
    expect(screen.getByRole('heading', { name: file.path })).toBeTruthy();
    expect(screen.getAllByRole('region', { name: 'XML of Gun_OutRifle.xml' })).toHaveLength(2);
  });
});
