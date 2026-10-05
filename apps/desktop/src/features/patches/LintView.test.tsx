import { fireEvent, screen } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import type { DesignerLintFilesResult } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import { LintView } from './LintView';
import { installTransport, openFixtureProject } from './testSupport';

function view() {
  const project = openFixtureProject('patches-project-converted');
  renderWithProviders(
    <LintView project={{ projectId: project.projectId, path: project.path, name: project.name }} />,
  );
}

describe('LintView', () => {
  it('groups the findings by file with the operation, the explanation and the rules that did not run', async () => {
    const transport = installTransport({
      designer_lint_files: () => loadFixture('designer_lint_files'),
    });
    view();
    expect(
      await screen.findByText('Files checked: 1. Errors: 1. Warnings: 10. Notes: 0.'),
    ).toBeTruthy();
    const file = screen.getByRole('region', { name: 'Patches/ce_patch.xml' });
    expect(file.textContent).toContain('1 operation');
    // the file headings sit directly under the page heading, level 2 like the other panels
    expect(screen.getByRole('heading', { level: 2, name: 'Patches/ce_patch.xml' })).toBeTruthy();
    expect(screen.getAllByText('CEP023').length).toBe(5);
    expect(screen.getAllByText('Operation 1').length).toBeGreaterThan(5);
    expect(screen.getAllByText('Explanation and location').length).toBeGreaterThan(5);
    expect(screen.getByText('2 rules did not run')).toBeTruthy();
    expect(transport.calls.filter((c) => c.name === 'designer_lint_files').length).toBe(1);
    expect(transport.calls.some((c) => c.name === 'designer_export_plan')).toBe(false);
  });

  it('shows the reason a rule did not run when the list is opened', async () => {
    installTransport({ designer_lint_files: () => loadFixture('designer_lint_files') });
    view();
    fireEvent.click(await screen.findByText('2 rules did not run'));
    expect(
      await screen.findByText('the type table of the installed Combat Extended was not supplied'),
    ).toBeTruthy();
  });

  it('says why a file was not checked', async () => {
    const result = loadFixture<DesignerLintFilesResult>('designer_lint_files');
    installTransport({
      designer_lint_files: () => ({
        ...result,
        ceData: false,
        files: [
          {
            path: 'Patches/bad.xml',
            status: 'parse-failed',
            bytes: 12,
            operations: 0,
            findings: [],
          },
        ],
        counts: { files: 1, checked: 0, errors: 0, warnings: 0, notes: 0 },
      }),
    });
    view();
    expect(await screen.findByText('Not valid XML')).toBeTruthy();
    expect(screen.getByText('This file was not checked.')).toBeTruthy();
    expect(screen.getByText(/Combat Extended is not loaded/)).toBeTruthy();
  });

  it('says so when the mod has no patch file', async () => {
    const result = loadFixture<DesignerLintFilesResult>('designer_lint_files');
    installTransport({ designer_lint_files: () => ({ ...result, files: [], project: [] }) });
    view();
    expect(await screen.findByText('Nothing to check')).toBeTruthy();
  });

  it('shows the error and checks again on retry', async () => {
    let calls = 0;
    installTransport({
      designer_lint_files: () => {
        calls += 1;
        if (calls === 1) throw { code: 'io.failed', message: 'disk', errorId: 'e1' };
        return loadFixture('designer_lint_files');
      },
    });
    view();
    expect(await screen.findByText('disk')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByText(/Files checked: 1/)).toBeTruthy();
  });
});
