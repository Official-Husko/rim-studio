import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createMockTransport, mockError } from 'rimstudio-testkit';
import { call, callCommand, getTransport, setTransport, transportKind } from './client';
import { isApiError, normalizeError } from './error';
import { jobs, resetJobs, setJobCommands } from './jobs';

beforeEach(() => resetJobs());
afterEach(() => resetJobs());

describe('call', () => {
  it('returns the response data of the mock transport', async () => {
    setTransport(createMockTransport({ handlers: { app_ping: () => ({ pong: true }) } }));
    expect(await call<{ pong: boolean }>('app_ping')).toEqual({ pong: true });
    expect(transportKind.value).toBe('mock');
  });

  it('sends the request body', async () => {
    const transport = createMockTransport({ handlers: { echo: (r) => r } });
    setTransport(transport);
    await call('echo', { a: 1 });
    expect(transport.calls[0]).toEqual({ name: 'echo', request: { a: 1 } });
  });

  it('defaults the request to an empty object', async () => {
    const transport = createMockTransport({ responses: { x: 1 } });
    setTransport(transport);
    await call('x');
    expect(transport.calls[0]?.request).toEqual({});
  });

  it('normalises an ApiError and keeps code and details', async () => {
    setTransport(
      createMockTransport({
        handlers: {
          fail: () =>
            Promise.reject(mockError('list.revision-conflict', 'conflict', { expectedRev: 1 })),
        },
      }),
    );
    await expect(call('fail')).rejects.toMatchObject({
      code: 'list.revision-conflict',
      details: { expectedRev: 1 },
      errorId: 'e-mock0001',
    });
  });

  it('turns any other failure into ipc.transport with an error id', async () => {
    setTransport(
      createMockTransport({
        handlers: { boom: () => Promise.reject(new TypeError('Failed to fetch')) },
      }),
    );
    const error = await call('boom').catch((e: unknown) => e);
    expect(isApiError(error)).toBe(true);
    expect(error).toMatchObject({ code: 'ipc.transport', message: 'Failed to fetch' });
    expect((error as { errorId: string }).errorId).toMatch(/^c-/);
  });

  it('rejects unknown commands with the mock error code', async () => {
    setTransport(createMockTransport());
    await expect(call('nope')).rejects.toMatchObject({ code: 'ipc.unknown-command' });
  });

  it('tracks job commands in the job store while they run', async () => {
    setJobCommands(['library_scan']);
    let release: () => void = () => {};
    setTransport(
      createMockTransport({
        handlers: {
          library_scan: () =>
            new Promise((r) => {
              release = () => r({ ok: true });
            }),
        },
      }),
    );
    const pending = call('library_scan');
    expect(jobs.value.map((j) => j.state)).toEqual(['running']);
    release();
    await pending;
    expect(jobs.value.map((j) => j.state)).toEqual(['done']);
  });

  it('marks a failed job call as failed', async () => {
    setJobCommands(['library_scan']);
    setTransport(
      createMockTransport({
        handlers: { library_scan: () => Promise.reject(mockError('x.y', 'no')) },
      }),
    );
    await call('library_scan').catch(() => undefined);
    expect(jobs.value[0]?.state).toBe('failed');
  });
});

describe('getTransport', () => {
  it('returns what was installed', () => {
    const t = createMockTransport();
    setTransport(t);
    expect(getTransport()).toBe(t);
  });
});

describe('normalizeError', () => {
  it('maps an aborted fetch to ipc.aborted', () => {
    expect(normalizeError(new DOMException('x', 'AbortError')).code).toBe('ipc.aborted');
  });
  it('wraps strings', () => {
    expect(normalizeError('plain').message).toBe('plain');
  });
});

describe('callCommand', () => {
  it('sends a typed request and returns the typed response', async () => {
    const transport = createMockTransport({
      handlers: { app_get_info: () => ({ version: '0.1.0' }) },
    });
    setTransport(transport);
    // app_get_info has an empty request, so the argument is optional
    const response = await callCommand('app_get_info');
    expect(response).toMatchObject({ version: '0.1.0' });
    expect(transport.calls[0]).toEqual({ name: 'app_get_info', request: {} });
    // a command with required request fields must be given its request
    // @ts-expect-error app_ping needs a request
    await callCommand('app_ping').catch(() => undefined);
  });
});
