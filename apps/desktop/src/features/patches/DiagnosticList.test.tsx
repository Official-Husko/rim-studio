import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { DiagnosticList } from './DiagnosticList';

describe('DiagnosticList', () => {
  it('writes the severity word, the code and the message', () => {
    render(
      <DiagnosticList
        label="Notes"
        diagnostics={[
          {
            code: 'ce.cep007-makegun-repeated',
            severity: 'error',
            message: 'converted more than once',
          },
          { code: 'ce.about-suggestion', severity: 'hint', message: 'add loadAfter' },
        ]}
      />,
    );
    const list = screen.getByRole('list', { name: 'Notes' });
    expect(list.querySelectorAll('li')).toHaveLength(2);
    expect(screen.getByText('Error')).toBeTruthy();
    expect(screen.getByText('Hint')).toBeTruthy();
    expect(screen.getByText('ce.cep007-makegun-repeated')).toBeTruthy();
    expect(screen.getByText('add loadAfter')).toBeTruthy();
  });
});
