import { describe, expect, it } from 'vitest';
import { groupDiagnostics, joinPath } from './model';
import { runScan, scanError, scanResult, scanning } from './scanStore';
import { installTransport } from './testSupport';
import { checkCombatExtended, ce } from './ceStore';
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

describe('Combat Extended lookup', () => {
  it('finds the workshop item of Combat Extended', async () => {
    const transport = installTransport({
      sources_probe_folder: () => loadFixture('probe-ce-workshop-item'),
    });
    await checkCombatExtended('/steam/workshop/content/294100');
    expect(ce.value).toMatchObject({ status: 'found', mods: 1 });
    expect(transport.calls[0]?.request).toEqual({
      path: '/steam/workshop/content/294100/2890901044',
    });
  });

  it('reports it missing when the folder is not there', async () => {
    installTransport({
      sources_probe_folder: () => ({
        kind: 'missing',
        modCount: 0,
        suggestedDepth: 1,
        suggestedLayout: 'auto',
        warnings: [],
        overlaps: [],
        diagnostics: [],
        canSave: false,
      }),
    });
    await checkCombatExtended('/w');
    expect(ce.value.status).toBe('missing');
  });

  it('joins paths with the separator in use', () => {
    expect(joinPath('C:\\steam\\content', '1')).toBe('C:\\steam\\content\\1');
    expect(joinPath('/a/b/', 'c')).toBe('/a/b/c');
  });
});
