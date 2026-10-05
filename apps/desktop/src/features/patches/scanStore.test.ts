import { beforeEach, describe, expect, it } from 'vitest';
import { loadFixture } from 'rimstudio-testkit';
import type { ProjectSummaryDto } from 'rimstudio-ipc-types';
import {
  candidates,
  checked,
  focused,
  runScan,
  scan,
  setAllChecked,
  setChecked,
} from './scanStore';
import { installTransport, openFixtureProject } from './testSupport';

beforeEach(() => {
  installTransport();
});

describe('scanStore', () => {
  it('scans, focuses the first convertible weapon and checks on request', async () => {
    const p = openFixtureProject();
    await runScan({ projectId: p.projectId, path: p.path, name: p.name });
    expect(scan.value.phase).toBe('ready');
    expect(candidates()).toHaveLength(5);
    expect(focused.value).toBe('OH_G41m');
    setChecked('OH_G41w', true);
    expect([...checked.value]).toEqual(['OH_G41w']);
    setAllChecked(true);
    expect(checked.value.size).toBe(5);
    setAllChecked(false);
    expect(checked.value.size).toBe(0);
  });

  it('keeps the old table while it scans again', async () => {
    const p = loadFixture<ProjectSummaryDto>('patches-project-plain');
    const ref = { projectId: p.projectId, path: p.path, name: p.name };
    await runScan(ref);
    const again = runScan(ref);
    const during = scan.value;
    expect(during.phase).toBe('loading');
    expect(during.phase === 'loading' && during.data?.candidates.length).toBe(5);
    await again;
    expect(scan.value.phase).toBe('ready');
  });
});
