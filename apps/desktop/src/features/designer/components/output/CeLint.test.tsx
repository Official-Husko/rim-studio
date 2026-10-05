import { screen } from '@testing-library/preact';
import type { DiagnosticDto, WritePlanDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { isLint } from '../../output-model';
import { fixture } from '../../testSupport';
import { CeLint } from './CeLint';

describe('CeLint', () => {
  it('shows the rules the real plan did not check', () => {
    const plan = fixture<WritePlanDto>('designer-output-plan-ce-ready');
    renderWithProviders(<CeLint lint={plan.diagnostics.filter(isLint)} />);
    expect(screen.getByText('Clean')).toBeTruthy();
    expect(screen.getByText('The patch has no findings.')).toBeTruthy();
    expect(screen.getByText('Not checked')).toBeTruthy();
    expect(screen.getAllByText(/was not checked/)).toHaveLength(2);
  });

  it('shows a finding with its rule code and the reason', () => {
    const finding: DiagnosticDto = {
      code: 'ce.cep011-toolce-no-penetration',
      severity: 'warning',
      message:
        'the tool stock has the Combat Extended class but no penetration; every armor blocks it',
    };
    renderWithProviders(<CeLint lint={[finding]} />);
    expect(screen.getByText('1 finding')).toBeTruthy();
    expect(screen.getByText('ce.cep011-toolce-no-penetration')).toBeTruthy();
    expect(screen.getByText(/every armor blocks it/)).toBeTruthy();
    expect(screen.getByText('Warning')).toBeTruthy();
  });
});
