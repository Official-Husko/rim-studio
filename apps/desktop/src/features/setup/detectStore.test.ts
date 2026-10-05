import { loadFixture } from 'rimstudio-testkit';
import type { DetectionReportDto } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import {
  detectAgain,
  detectError,
  loadReport,
  overridePath,
  report,
  reportLoaded,
} from './detectStore';
import { installTransport } from './testSupport';

describe('detectStore', () => {
  it('stays empty on a first run', async () => {
    installTransport({ detect_get_report: () => ({}) });
    await loadReport();
    expect(report.value).toBeUndefined();
    expect(reportLoaded.value).toBe(true);
  });

  it('loads the cached report', async () => {
    installTransport();
    await loadReport();
    expect(report.value?.installs[0]?.version?.raw).toBe('1.6.4871 rev598');
    expect(report.value?.warnings.map((w) => w.code)).toContain('steam.manifest-leftover');
  });

  it('runs detection with force and reloads the sources', async () => {
    const transport = installTransport();
    await detectAgain();
    expect(report.value?.selected.install).toBe('steam:/home/pawbeans/.local/share/Steam');
    expect(transport.calls.map((c) => c.name)).toEqual([
      'detect_run',
      'sources_list',
      'settings_get',
    ]);
    expect(transport.calls[0]?.request).toEqual({ force: true });
  });

  it('sends the override path, and no path to clear it', async () => {
    const transport = installTransport();
    await overridePath('game-install', '/games/RimWorld');
    await overridePath('user-dir', undefined);
    expect(transport.calls[0]?.request).toEqual({ field: 'game-install', path: '/games/RimWorld' });
    expect(
      transport.calls.find((c) => (c.request as { field?: string }).field === 'user-dir')?.request,
    ).toEqual({
      field: 'user-dir',
    });
  });

  it('keeps the error of a failed detection', async () => {
    installTransport({
      detect_run: () => {
        throw { code: 'detect.failed', message: 'no steam', errorId: 'e1' };
      },
    });
    await detectAgain();
    expect(detectError.value?.code).toBe('detect.failed');
    expect((loadFixture('detect-run') as DetectionReportDto).schema).toBe(1);
  });
});
