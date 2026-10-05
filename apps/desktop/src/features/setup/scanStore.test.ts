import { describe, expect, it } from 'vitest';
import { groupDiagnostics } from './model';
import { runScan, scanError, scanResult, scanning } from './scanStore';
import { installTransport } from './testSupport';
import { loadFixture } from 'rimstudio-testkit';
import type { LibraryScanResult } from 'rimstudio-ipc-types';

describe('scanStore', () => {
  it('runs a scan, keeps the result and reloads the sources', async () => {
    const transport = installTransport();
    const pendingRun = runScan(false);
    expect(scanning.value).toBe('scan');
    await pendingRun;
    expect(scanning.value).toBeUndefined();
    expect(scanResult.value?.stats.modsFound).toBe(744);
    expect(transport.calls[0]).toEqual({ name: 'library_scan', request: { full: false } });
    expect(transport.calls.some((c) => c.name === 'sources_list')).toBe(true);
  });

  it('asks for a full scan', async () => {
    const transport = installTransport();
    await runScan(true);
    expect(transport.calls[0]?.request).toEqual({ full: true });
  });

  it('keeps the error of a failed scan', async () => {
    installTransport({
      library_scan: () => {
        throw { code: 'library.scan-failed', message: 'disk', errorId: 'e3' };
      },
    });
    await runScan(false);
    expect(scanError.value?.code).toBe('library.scan-failed');
  });
});

describe('diagnostic groups', () => {
  it('groups the real scan summary by code, worst and largest first', () => {
    const result = loadFixture<LibraryScanResult>('library-scan');
    const groups = groupDiagnostics(result.diagnostics);
    expect(groups.map((g) => [g.code, g.count, g.samples.length])).toEqual([
      ['loadfolders.ignored-attribute', 9, 9],
      ['about.no-supported-versions', 1, 1],
    ]);
  });
});

describe('scan facts', () => {
  it('keeps the counts, the duplicate groups and the Combat Extended entry of the result', async () => {
    installTransport({ library_scan: () => loadFixture('library-scan-with-custom') });
    await runScan(false);
    const result = scanResult.value;
    expect(result?.counts).toMatchObject({ mods: 763, loadable: 744, customOnly: 19 });
    expect(result?.duplicates?.total).toBe(6);
    expect(result?.ceInLibrary).toMatchObject({ present: true, version: '16.7.3.0' });
  });
});
