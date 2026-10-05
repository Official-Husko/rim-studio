import { render, screen, waitFor } from '@testing-library/preact';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IpcScope, useIpcScope } from './lifetime';
import { clearQueries, invalidate, query, queryCacheSize, setQueryClock, useQuery } from './query';

let now = 0;
beforeEach(() => {
  clearQueries();
  now = 0;
  setQueryClock(() => now);
});

const deferred = <T,>() => {
  let resolve: (v: T) => void = () => {};
  let reject: (e: unknown) => void = () => {};
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
};

describe('query', () => {
  it('loads once and shares the result by key', async () => {
    const fetcher = vi.fn().mockResolvedValue('v1');
    const a = query('k', fetcher);
    const b = query('k', fetcher);
    expect(a.loading.value).toBe(true);
    await a.refresh().catch(() => undefined);
    expect(a.data.value).toBe('v1');
    expect(b.data.value).toBe('v1');
    expect(a.data).toBe(b.data);
  });

  it('serves fresh data without fetching and refetches when stale', async () => {
    const fetcher = vi.fn().mockResolvedValueOnce('v1').mockResolvedValueOnce('v2');
    const first = query('k', fetcher, { staleMs: 1000 });
    await vi.waitFor(() => expect(first.data.value).toBe('v1'));
    now = 500;
    query('k', fetcher, { staleMs: 1000 });
    expect(fetcher).toHaveBeenCalledTimes(1);
    now = 2000;
    const again = query('k', fetcher, { staleMs: 1000 });
    expect(again.data.value).toBe('v1'); // stale data stays visible while it reloads
    expect(again.loading.value).toBe(true);
    await vi.waitFor(() => expect(again.data.value).toBe('v2'));
  });

  it('exposes a normalised error and keeps the previous data', async () => {
    const fetcher = vi
      .fn()
      .mockResolvedValueOnce('v1')
      .mockRejectedValueOnce({ code: 'io.not-a-directory', message: 'no', errorId: 'e-1' });
    const handle = query('k', fetcher, { staleMs: 0 });
    await vi.waitFor(() => expect(handle.data.value).toBe('v1'));
    await handle.refresh();
    expect(handle.error.value?.code).toBe('io.not-a-directory');
    expect(handle.data.value).toBe('v1');
    expect(handle.loading.value).toBe(false);
  });

  it('invalidate reruns held keys', async () => {
    const fetcher = vi.fn().mockResolvedValueOnce('v1').mockResolvedValueOnce('v2');
    const scope = new IpcScope();
    const handle = query('proj:1', fetcher, { scope });
    await vi.waitFor(() => expect(handle.data.value).toBe('v1'));
    invalidate(/^proj:/);
    await vi.waitFor(() => expect(handle.data.value).toBe('v2'));
    expect(fetcher).toHaveBeenCalledTimes(2);
  });

  it('invalidate only marks an unheld scoped key as stale', async () => {
    const fetcher = vi.fn().mockResolvedValue('v');
    const scope = new IpcScope();
    const handle = query('k', fetcher, { scope });
    await vi.waitFor(() => expect(handle.data.value).toBe('v'));
    scope.dispose();
    invalidate('k');
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it('drops a result that arrives after its only holder went away', async () => {
    const d = deferred<string>();
    const scope = new IpcScope();
    const handle = query('late', () => d.promise, { scope });
    scope.dispose();
    d.resolve('too late');
    await new Promise((r) => setTimeout(r, 0));
    expect(handle.data.value).toBeUndefined();
  });

  it('clears the cache', () => {
    query('a', () => Promise.resolve(1));
    expect(queryCacheSize()).toBe(1);
    clearQueries();
    expect(queryCacheSize()).toBe(0);
  });
});

describe('useQuery', () => {
  function Probe({ id, fetcher }: { id: string; fetcher: () => Promise<string> }) {
    const q = useQuery(`probe:${id}`, fetcher);
    return <p>{q.loading.value ? 'loading' : (q.data.value ?? q.error.value?.code ?? 'none')}</p>;
  }

  it('renders the loading then the loaded state', async () => {
    render(<Probe id="1" fetcher={() => Promise.resolve('loaded')} />);
    expect(screen.getByText('loading')).toBeTruthy();
    await waitFor(() => expect(screen.getByText('loaded')).toBeTruthy());
  });

  it('shows the error code', async () => {
    render(
      <Probe
        id="2"
        fetcher={() => Promise.reject({ code: 'x.fail', message: 'm', errorId: 'e' })}
      />,
    );
    await waitFor(() => expect(screen.getByText('x.fail')).toBeTruthy());
  });

  it('drops the late result after unmount', async () => {
    const d = deferred<string>();
    const { unmount } = render(<Probe id="3" fetcher={() => d.promise} />);
    unmount();
    d.resolve('late');
    await new Promise((r) => setTimeout(r, 0));
    const again = query('probe:3', () => Promise.resolve('fresh'), { staleMs: 0 });
    expect(again.data.value).toBeUndefined();
  });
});

describe('IpcScope', () => {
  it('runs disposers once in reverse order and runs late additions at once', () => {
    const order: number[] = [];
    const scope = new IpcScope();
    scope.add(() => order.push(1));
    scope.add(() => order.push(2));
    scope.dispose();
    scope.dispose();
    scope.add(() => order.push(3));
    expect(order).toEqual([2, 1, 3]);
    expect(scope.disposed).toBe(true);
  });

  it('is disposed when the component unmounts', () => {
    const seen: IpcScope[] = [];
    function Holder() {
      seen.push(useIpcScope());
      return null;
    }
    const { unmount } = render(<Holder />);
    expect(seen[0]?.disposed).toBe(false);
    unmount();
    expect(seen[0]?.disposed).toBe(true);
  });
});
