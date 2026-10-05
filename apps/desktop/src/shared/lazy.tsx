import type { ComponentType } from 'preact';
import { useEffect, useState } from 'preact/hooks';
import { Spinner } from 'rimstudio-ui';

/**
 * Wrap a dynamic import so a page loads on first render and is its own chunk. Features export
 * their page through this from index.ts. (No preact/compat: a dozen lines are enough.)
 */
export function lazyPage<P extends object>(
  loader: () => Promise<{ default: ComponentType<P> }>,
): ComponentType<P> {
  let loaded: ComponentType<P> | undefined;
  let pending: Promise<void> | undefined;
  const load = (): Promise<void> => {
    pending ??= loader().then((m) => {
      loaded = m.default;
    });
    return pending;
  };
  return function LazyPage(props: P) {
    const [, force] = useState(0);
    const [failure, setFailure] = useState<unknown>();
    useEffect(() => {
      if (loaded) return;
      load().then(
        () => force((n) => n + 1),
        (e: unknown) => setFailure(e),
      );
    }, []);
    if (failure) throw failure;
    if (!loaded) {
      return (
        <div class="flex h-full items-center justify-center text-muted">
          <Spinner size="md" />
        </div>
      );
    }
    const Page = loaded;
    return <Page {...props} />;
  };
}
