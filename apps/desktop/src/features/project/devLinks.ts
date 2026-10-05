type DevLink = 'open' | 'file' | 'tab' | 'new' | 'parent';

/**
 * Development deep links of the project page, read from the hash query: #/project?open=PATH opens a
 * project, file=REL shows a file once it is loaded, tab=layout|guide picks a tab, new=1 opens the
 * new mod dialog and parent=PATH fills its folder. Used for screenshots; production builds ignore them.
 */
export function devLink(name: DevLink): string | undefined {
  if (!import.meta.env.DEV) return undefined;
  const query = window.location.hash.split('?')[1];
  return query ? (new URLSearchParams(query).get(name) ?? undefined) : undefined;
}
