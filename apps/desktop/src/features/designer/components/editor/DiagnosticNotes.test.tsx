import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { DiagnosticNotes, severityText } from './DiagnosticNotes';

describe('DiagnosticNotes', () => {
  it('lists warnings and info with the code and leaves errors to the field', () => {
    renderWithProviders(
      <DiagnosticNotes
        diagnostics={[
          { code: 'design.pool-thin', severity: 'info', message: 'few items' },
          { code: 'design.required-missing', severity: 'error', message: 'missing' },
        ]}
      />,
    );
    expect(screen.getByText(/few items/)).toBeTruthy();
    expect(screen.getByText('Info')).toBeTruthy();
    expect(screen.queryByText(/missing/)).toBeNull();
  });

  it('names every severity', () => {
    expect(severityText('error')).toBe('Error');
    expect(severityText('warning')).toBe('Warning');
    expect(severityText('hint')).toBe('Hint');
  });
});
