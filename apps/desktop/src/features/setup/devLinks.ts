/**
 * Development deep links of the setup page, read from the hash query: #/setup?scan=run starts a
 * scan on load and #/setup?add=PATH opens the add folder dialog for PATH. Used for screenshots;
 * production builds ignore them.
 */
export function devLink(name: 'scan' | 'add'): string | undefined {
  if (!import.meta.env.DEV) return undefined;
  const query = location.hash.split('?')[1];
  return query ? (new URLSearchParams(query).get(name) ?? undefined) : undefined;
}
