import { signal } from '@preact/signals';

export type RouteId = 'setup' | 'project' | 'weapons' | 'patches' | 'gallery';

export const ROUTE_IDS: readonly RouteId[] = ['setup', 'project', 'weapons', 'patches', 'gallery'];
const DEFAULT_ROUTE: RouteId = 'setup';

/** Read the route from a location hash such as "#/weapons"; unknown values give the default. */
export function parseHash(hash: string): RouteId {
  const id = hash.replace(/^#\/?/, '').split(/[/?]/)[0];
  return (ROUTE_IDS as readonly string[]).includes(id ?? '') ? (id as RouteId) : DEFAULT_ROUTE;
}

/** The query part of the hash, for example "#/gallery?dialog=open" gives dialog=open. */
export function parseParams(hash: string): URLSearchParams {
  const at = hash.indexOf('?');
  return new URLSearchParams(at < 0 ? '' : hash.slice(at + 1));
}

const initialHash = typeof location === 'undefined' ? '' : location.hash;

/** The visible tool. Deep links are not needed beyond the hash. */
export const route = signal<RouteId>(parseHash(initialHash));

/** Query parameters of the hash; a few development views read them (open a drawer, a dialog). */
export const routeParams = signal<URLSearchParams>(parseParams(initialHash));

/** Go to a tool by writing the hash; the hashchange listener updates the signal. */
export function navigate(id: RouteId): void {
  if (location.hash !== `#/${id}`) location.hash = `/${id}`;
  route.value = id;
}

/** Start listening to the browser hash. Returns the stop function. */
export function startRouter(): () => void {
  const onChange = (): void => {
    route.value = parseHash(location.hash);
    routeParams.value = parseParams(location.hash);
  };
  window.addEventListener('hashchange', onChange);
  onChange();
  return () => window.removeEventListener('hashchange', onChange);
}
