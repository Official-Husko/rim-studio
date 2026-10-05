import { describe, expect, it } from 'vitest';
import {
  createMockTransport,
  fixtureNames,
  loadFixture,
  mockError,
  renderWithProviders,
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

describe('renderWithProviders', () => {
  it('sets the theme attributes and renders', () => {
    const { getByText } = renderWithProviders(<p>hello</p>, { density: 'compact' });
    expect(getByText('hello')).toBeTruthy();
    expect(document.documentElement.getAttribute('data-density')).toBe('compact');
  });
});
