import { useEffect, useRef } from 'preact/hooks';

type Disposer = () => void;

/** Owns the disposers of everything a component started (queries, streams, job watchers). */
export class IpcScope {
  private disposers: Disposer[] = [];
  private closed = false;

  /** True once the scope has been disposed. */
  get disposed(): boolean {
    return this.closed;
  }

  /** Register a disposer; it runs at once when the scope is already disposed. */
  add(disposer: Disposer): void {
    if (this.closed) disposer();
    else this.disposers.push(disposer);
  }

  /** Run every disposer once, in reverse order. */
  dispose(): void {
    if (this.closed) return;
    this.closed = true;
    for (const d of this.disposers.reverse()) d();
    this.disposers = [];
  }
}

/** A scope that lives as long as the calling component; disposed on unmount. */
export function useIpcScope(): IpcScope {
  const ref = useRef<IpcScope>();
  if (!ref.current) ref.current = new IpcScope();
  const scope = ref.current;
  useEffect(
    () => () => {
      scope.dispose();
      ref.current = undefined;
    },
    [scope],
  );
  return scope;
}
