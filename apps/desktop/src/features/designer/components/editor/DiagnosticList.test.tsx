import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { PreviewDto } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import { DiagnosticList } from './DiagnosticList';

describe('DiagnosticList', () => {
  it('lists the errors of a blank melee weapon first, with the codes', () => {
    const preview = fixture<PreviewDto>('designer-preview-melee');
    renderWithProviders(
      <DiagnosticList diagnostics={preview.diagnostics} onFocusField={() => undefined} />,
    );
    expect(screen.getByText(/errors$/)).toBeTruthy();
    expect(screen.getAllByText('design.required-missing').length).toBeGreaterThan(0);
  });

  it('jumps to the field of a diagnostic', () => {
    const onFocus = vi.fn();
    renderWithProviders(
      <DiagnosticList
        diagnostics={[
          {
            code: 'design.required-missing',
            severity: 'error',
            message: 'mass is required',
            field: '/mass',
          },
        ]}
        onFocusField={onFocus}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Go to field' }));
    expect(onFocus).toHaveBeenCalledWith('/mass');
  });

  it('says when nothing needs attention', () => {
    renderWithProviders(<DiagnosticList diagnostics={[]} onFocusField={() => undefined} />);
    expect(screen.getByText('Nothing to report.')).toBeTruthy();
    expect(screen.getByText('No errors')).toBeTruthy();
  });
});
