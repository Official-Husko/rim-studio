import { render, screen } from '@testing-library/preact';
import { loadFixture } from 'rimstudio-testkit';
import type { ApplyReportDto } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import { ApplyReport } from './ApplyReport';

const report = loadFixture<ApplyReportDto>('patches-apply-report');

describe('ApplyReport', () => {
  it('lists the written files with the read back and the dry apply', () => {
    render(<ApplyReport results={[{ defName: 'OH_G41m', report }]} />);
    expect(screen.getByText('1 weapon applied')).toBeTruthy();
    expect(screen.getByText('Patch test passed')).toBeTruthy();
    expect(screen.getAllByText('Read back')).toHaveLength(2);
    expect(screen.getByText('LoadFolders.xml')).toBeTruthy();
  });

  it('shows the backup path of a replaced file', () => {
    const replaced: ApplyReportDto = {
      ...report,
      written: [
        {
          path: 'a.xml',
          action: 'update-region',
          bytes: 3,
          backupPath: '/data/project-backups/p/a.bak',
          verified: true,
        },
      ],
    };
    render(<ApplyReport results={[{ defName: 'W', report: replaced }]} />);
    expect(screen.getByText('/data/project-backups/p/a.bak')).toBeTruthy();
  });

  it('shows weapons that were not applied and the error', () => {
    render(
      <ApplyReport
        results={[
          { defName: 'A', report },
          {
            defName: 'B',
            error: { code: 'designer.apply-failed', message: 'disk full', errorId: 'e' },
          },
          { defName: 'C', skipped: 'not-ready' },
        ]}
      />,
    );
    expect(
      screen.getByText('Some weapons were not applied. What was written stays in the mod.'),
    ).toBeTruthy();
    expect(screen.getByText('designer.apply-failed')).toBeTruthy();
    expect(screen.getAllByText('Not applied')).toHaveLength(2);
  });
});
