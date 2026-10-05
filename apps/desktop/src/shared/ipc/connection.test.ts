import { afterEach, describe, expect, it, vi } from 'vitest';
import { connect, connection } from './connection';
import { transportKind } from './client';
import { createHttpTransport } from './transports/http';
import { createTauriTransport } from './transports/tauri';

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('connect', () => {
  it('falls back to the mock transport when no bridge answers', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('refused')));
    const kind = await connect({ timeoutMs: 200 });
    expect(kind).toBe('mock');
    expect(connection.value).toBe('mock');
    expect(transportKind.value).toBe('mock');
  });

  it('can be forced to mock', async () => {
    expect(await connect({ forceMock: true })).toBe('mock');
  });

  it('uses the bridge when /dev/health answers', async () => {
    const fetchMock = vi.fn(async (url: string) => {
      if (url.endsWith('/dev/health')) return new Response('{"ok":true}', { status: 200 });
      if (url.endsWith('/dev/commands'))
        return new Response('[{"name":"library_scan","kind":"job","request":"R","response":"S"}]', {
          status: 200,
        });
      return new Response('{}', { status: 200 });
    });
    vi.stubGlobal('fetch', fetchMock);
    class FakeEventSource {
      onmessage: ((e: MessageEvent<string>) => void) | null = null;
      close(): void {}
    }
    vi.stubGlobal('EventSource', FakeEventSource);
    expect(await connect()).toBe('bridge');
    expect(connection.value).toBe('bridge');
  });
});

describe('http transport', () => {
  const respond = (body: unknown, status = 200) =>
    vi.fn().mockResolvedValue(new Response(JSON.stringify(body), { status }));

  it('posts the request and unwraps the envelope', async () => {
    const fetchMock = respond({ ok: true, data: { n: 1 } });
    vi.stubGlobal('fetch', fetchMock);
    const t = createHttpTransport();
    expect(await t.call('app_ping', { a: 1 })).toEqual({ n: 1 });
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('/rpc/app_ping');
    expect(init.method).toBe('POST');
    expect(new Headers(init.headers).get('content-type')).toBe('application/json');
    expect(init.body).toBe('{"a":1}');
  });

  it('rejects with the error of a failed envelope', async () => {
    vi.stubGlobal(
      'fetch',
      respond({
        ok: false,
        error: { code: 'designer.bad-spec', message: 'm', errorId: 'e-1', details: { field: 'x' } },
      }),
    );
    await expect(createHttpTransport().call('designer_preview', {})).rejects.toMatchObject({
      code: 'designer.bad-spec',
      details: { field: 'x' },
    });
  });

  it('turns a protocol error status into ipc.protocol', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('Forbidden', { status: 403 })));
    await expect(createHttpTransport().call('x', {})).rejects.toMatchObject({
      code: 'ipc.protocol',
    });
  });

  it('accepts a bare error body of a 4xx response', async () => {
    vi.stubGlobal(
      'fetch',
      respond({ code: 'ipc.unknown-command', message: 'm', errorId: 'e' }, 404),
    );
    await expect(createHttpTransport().call('x', {})).rejects.toMatchObject({
      code: 'ipc.unknown-command',
    });
  });

  it('reads dev routes as plain JSON', async () => {
    vi.stubGlobal('fetch', respond({ bridgeVersion: '0.1.0' }));
    expect(await createHttpTransport().dev('/dev/info')).toEqual({ bridgeVersion: '0.1.0' });
  });
});

describe('tauri stub', () => {
  it('fails with a clear message', async () => {
    const t = createTauriTransport();
    await expect(t.call('x', {})).rejects.toMatchObject({
      code: 'ipc.transport',
      message: expect.stringContaining('shell'),
    });
  });
});
