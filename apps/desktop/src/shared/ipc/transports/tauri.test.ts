import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createTauriTransport, JOB_EVENT } from './tauri';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

const invokeMock = vi.mocked(invoke);
const listenMock = vi.mocked(listen);

beforeEach(() => {
  invokeMock.mockReset();
  listenMock.mockReset();
});

describe('tauri transport', () => {
  it('sends a call to rs_call with the name and the request', async () => {
    invokeMock.mockResolvedValue({ echo: 'hi' });
    const data = await createTauriTransport().call('app_ping', { echo: 'hi' });
    expect(data).toEqual({ echo: 'hi' });
    expect(invokeMock).toHaveBeenCalledWith('rs_call', {
      name: 'app_ping',
      request: { echo: 'hi' },
    });
  });

  it('sends an empty object for a call without a request', async () => {
    invokeMock.mockResolvedValue({});
    await createTauriTransport().call('app_get_info', undefined);
    expect(invokeMock).toHaveBeenCalledWith('rs_call', { name: 'app_get_info', request: {} });
  });

  it('passes the error envelope of a failed call through', async () => {
    const envelope = { code: 'ipc.unknown-command', message: 'no', errorId: 'e-1' };
    invokeMock.mockRejectedValue(envelope);
    await expect(createTauriTransport().call('x', {})).rejects.toBe(envelope);
  });

  it('stops waiting when the signal aborts', async () => {
    invokeMock.mockReturnValue(new Promise(() => undefined));
    const controller = new AbortController();
    const pending = createTauriTransport().call('library_scan', {}, controller.signal);
    controller.abort();
    await expect(pending).rejects.toMatchObject({ name: 'AbortError' });
    await expect(
      createTauriTransport().call('library_scan', {}, controller.signal),
    ).rejects.toMatchObject({ name: 'AbortError' });
  });

  it('maps the dev routes to the info and command list commands', async () => {
    invokeMock.mockResolvedValue({ platform: 'linux' });
    const t = createTauriTransport();
    expect(await t.dev('/dev/health')).toEqual({ ok: true });
    expect(await t.dev('/dev/info')).toEqual({ platform: 'linux' });
    expect(invokeMock).toHaveBeenLastCalledWith('rs_info');
    await t.dev('/dev/commands');
    expect(invokeMock).toHaveBeenLastCalledWith('rs_commands');
  });

  it('refuses the folder browser routes of the bridge', async () => {
    await expect(createTauriTransport().dev('/dev/fs/list?path=%2F')).rejects.toMatchObject({
      code: 'platform.unavailable',
    });
  });

  it('delivers job events and stops listening on unsubscribe', async () => {
    const unlisten = vi.fn();
    let deliver: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementation(async (_name, handler) => {
      deliver = handler as (event: { payload: unknown }) => void;
      return unlisten;
    });
    const seen: unknown[] = [];
    const stop = createTauriTransport().events((e) => seen.push(e));
    await vi.waitFor(() => expect(deliver).toBeDefined());
    expect(listenMock).toHaveBeenCalledWith(JOB_EVENT, expect.any(Function));
    const event = { type: 'job-finished', jobId: 'j', command: 'detect_run', ok: true };
    deliver?.({ payload: event });
    expect(seen).toEqual([event]);
    stop();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('unlistens at once when unsubscribed before the listener is ready', async () => {
    const unlisten = vi.fn();
    listenMock.mockResolvedValue(unlisten);
    createTauriTransport().events(() => undefined)();
    await vi.waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });
});
