import { screen } from '@testing-library/preact';
import type { ApplyReportDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { fixture } from '../../testSupport';
import { ApplyResult } from './ApplyResult';

describe('ApplyResult', () => {
  it('lists the written files, the verification and the dry run of a first write', () => {
    renderWithProviders(
      <ApplyResult report={fixture<ApplyReportDto>('designer-output-apply-created')} />,
    );
    expect(screen.getByText('Wrote 3 files')).toBeTruthy();
    expect(screen.getByText('Every file of the plan was written.')).toBeTruthy();
    expect(
      screen.getByText('Compat/CombatExtended/Patches/outmodtwo_Weapons_Ranged.xml'),
    ).toBeTruthy();
    expect(screen.getAllByText('Verified')).toHaveLength(3);
    expect(
      screen.getByText('The Combat Extended patch loads against the definitions.'),
    ).toBeTruthy();
    expect(screen.getByText('No file was replaced, so no backup was needed.')).toBeTruthy();
  });

  it('shows the backup of a replaced file and the files left alone', () => {
    renderWithProviders(
      <ApplyResult report={fixture<ApplyReportDto>('designer-output-apply-updated')} />,
    );
    expect(screen.getByText('Wrote 1 file')).toBeTruthy();
    expect(screen.getByText('2 files were already up to date.')).toBeTruthy();
    expect(
      screen.getByText(/^Backup: \/home\/user\/\.local\/share\/rimstudio\/project-backups/),
    ).toBeTruthy();
    expect(
      screen.getByText(/^Backups are in \/home\/user\/\.local\/share\/rimstudio\/project-backups/),
    ).toBeTruthy();
    expect(screen.getByText('Update')).toBeTruthy();
  });

  it('reports a dry run that failed', () => {
    const base = fixture<ApplyReportDto>('designer-output-apply-created');
    renderWithProviders(
      <ApplyResult
        report={{
          ...base,
          dryApplyOk: false,
          diagnostics: [
            { code: 'ce.dry-run-failed', severity: 'error', message: 'the patch failed to apply' },
          ],
        }}
      />,
    );
    expect(screen.getByText('The patch did not load cleanly')).toBeTruthy();
    expect(screen.getByText(/the patch failed to apply/)).toBeTruthy();
  });
});
