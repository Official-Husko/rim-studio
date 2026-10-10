import { describe, expect, it } from 'vitest';
import {
  createMockTransport,
  fixtureNames,
  loadFixture,
  mockError,
  renderWithProviders,
  variantAnswer,
} from './index';

describe('fixtures', () => {
  it('loads json files from the fixtures folder', () => {
    expect(fixtureNames()).toContain('dev-info');
    expect(loadFixture<{ platform: string }>('dev-info').platform).toBe('linux');
  });

  it('returns a fresh copy each time', () => {
    const a = loadFixture<{ platform: string }>('dev-info');
    a.platform = 'changed';
    expect(loadFixture<{ platform: string }>('dev-info').platform).toBe('linux');
  });

  it('throws for an unknown fixture', () => {
    expect(() => loadFixture('nope')).toThrow(/nope/);
  });
});

describe('mock transport', () => {
  it('prefers handlers, then responses, then fixtures', async () => {
    const t = createMockTransport({
      handlers: { a: () => 'handler' },
      responses: { a: 'response', b: 'response-b' },
    });
    expect(await t.call('a', {})).toBe('handler');
    expect(await t.call('b', {})).toBe('response-b');
    expect(t.calls.map((c) => c.name)).toEqual(['a', 'b']);
  });

  it('rejects unknown commands with a coded error', async () => {
    const t = createMockTransport();
    await expect(t.call('nope', {})).rejects.toMatchObject({ code: 'ipc.unknown-command' });
  });

  it('serves the dev routes from fixtures', async () => {
    const t = createMockTransport();
    const home = (await t.dev('/dev/fs/home')) as { home: string };
    const list = (await t.dev(`/dev/fs/list?path=${encodeURIComponent(home.home)}`)) as {
      entries: unknown[];
    };
    expect(list.entries.length).toBeGreaterThan(0);
    await expect(t.dev('/dev/fs/list?path=/missing')).rejects.toMatchObject({
      code: 'io.not-a-directory',
    });
  });

  it('fans events out to subscribers until they unsubscribe', () => {
    const t = createMockTransport();
    const seen: string[] = [];
    const off = t.events((e) => seen.push(e.type));
    t.emit({ type: 'job-finished', jobId: 'j', command: 'c', ok: true });
    off();
    t.emit({ type: 'job-finished', jobId: 'j', command: 'c', ok: true });
    expect(seen).toEqual(['job-finished']);
    expect(mockError('x.y', 'm').errorId).toMatch(/^e-/);
  });
});

describe('request variants', () => {
  const removal = (defName: string) => ({
    projectId: 'p',
    draft: {},
    convert: { defName, answers: {}, remove: true },
  });

  it('answers the removal of a generated patch with a plan that cuts the weapon out of its file', async () => {
    const t = createMockTransport();
    const plan = (await t.call('designer_export_plan', removal('OH_G41m'))) as {
      files: Array<{ action: string; diff: string }>;
      hasErrors: boolean;
    };
    expect(plan.hasErrors).toBe(false);
    expect(plan.files).toHaveLength(1);
    expect(plan.files[0]?.action).toBe('update-region');
    expect(plan.files[0]?.diff).toContain('-  <!-- ====== OH_G41m ======');
  });

  it('answers the removal of a weapon with no generated patch with the error of the backend', async () => {
    const t = createMockTransport();
    const plan = (await t.call('designer_export_plan', removal('OH_G41w'))) as {
      files: unknown[];
      diagnostics: Array<{ code: string }>;
      hasErrors: boolean;
    };
    expect(plan.hasErrors).toBe(true);
    expect(plan.files).toEqual([]);
    expect(plan.diagnostics[0]?.code).toBe('designer.remove-nothing');
  });

  it('answers the apply of a removal with the report of the written file and its backup', async () => {
    const t = createMockTransport();
    const report = (await t.call('designer_apply_plan', {
      planId: 'x',
      request: removal('OH_G41m'),
      backup: true,
      dryApply: false,
    })) as { written: Array<{ action: string; backupPath?: string }> };
    expect(report.written[0]?.action).toBe('update-region');
    expect(report.written[0]?.backupPath).toContain('project-backups');
  });

  it('leaves every other request to the handlers and the fixtures', async () => {
    expect(variantAnswer('designer_export_plan', { convert: { defName: 'A', answers: {} } })).toBe(
      undefined,
    );
    expect(variantAnswer('designer_apply_plan', { planId: 'x', request: {} })).toBe(undefined);
    expect(variantAnswer('designer_convert_scan', removal('OH_G41m'))).toBe(undefined);
    const t = createMockTransport({
      handlers: { designer_export_plan: () => 'handler wins' },
    });
    expect(await t.call('designer_export_plan', removal('OH_G41m'))).toBe('handler wins');
  });
});

describe('renderWithProviders', () => {
  it('sets the theme attributes and renders', () => {
    const { getByText } = renderWithProviders(<p>hello</p>, { density: 'compact' });
    expect(getByText('hello')).toBeTruthy();
    expect(document.documentElement.getAttribute('data-density')).toBe('compact');
  });
});
