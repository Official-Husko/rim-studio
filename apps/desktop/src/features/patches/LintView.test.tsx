import { screen, waitFor } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { LintView } from './LintView';
import { installTransport, openFixtureProject } from './testSupport';

describe('LintView', () => {
  it('lists the findings of every converted weapon and the rules that did not run', async () => {
    const transport = installTransport({
      designer_convert_scan: () => loadFixture('patches-scan-converted'),
      designer_export_plan: () => ({
        planId: 'x',
        files: [],
        hasErrors: false,
        diagnostics: [
          {
            code: 'ce.cep007-makegun-repeated',
            severity: 'error',
            message: 'the def OH_G41m is converted more than once',
          },
          {
            code: 'ce.not-checked',
            severity: 'info',
            message: 'CEP010 was not checked: the type table was not supplied',
          },
        ],
      }),
    });
    const project = openFixtureProject('patches-project-converted');
    renderWithProviders(
      <LintView
        project={{ projectId: project.projectId, path: project.path, name: project.name }}
      />,
    );
    expect(
      await screen.findByText('Converted weapons checked: 5. Errors: 5. Warnings: 0.'),
    ).toBeTruthy();
    await waitFor(() => expect(screen.getAllByText('ce.cep007-makegun-repeated').length).toBe(5));
    expect(screen.getByText('1 rule did not run')).toBeTruthy();
    expect(transport.calls.filter((c) => c.name === 'designer_export_plan').length).toBe(5);
  });

  it('says so when no weapon carries a conversion', async () => {
    installTransport();
    const project = openFixtureProject();
    renderWithProviders(
      <LintView
        project={{ projectId: project.projectId, path: project.path, name: project.name }}
      />,
    );
    expect(await screen.findByText('Nothing to check')).toBeTruthy();
  });
});
