import type { E2eEnv } from './env.ts';

/** Calls a command of the bridge directly (test setup and checks that must not go through the page). */
export async function rpc<T = unknown>(env: E2eEnv, name: string, body: object = {}): Promise<T> {
  const response = await fetch(`${env.bridgeUrl}/rpc/${name}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', 'x-rimstudio-token': env.token },
    body: JSON.stringify(body),
  });
  const answer = (await response.json()) as { ok: boolean; data?: T; error?: { message: string } };
  if (!answer.ok) throw new Error(`${name} failed: ${answer.error?.message ?? 'unknown'}`);
  return answer.data as T;
}

/** The request of project_create with the scaffold defaults and the given overrides. */
export function createRequest(path: string, name: string, packageId: string, extra: object = {}): object {
  return {
    path,
    name,
    packageId,
    author: 'qa',
    description: '',
    supportedVersions: ['1.6'],
    versionedFolders: false,
    patchesFolder: true,
    languagesFolder: false,
    assembliesFolder: false,
    cePatchFolder: false,
    placeholderFiles: false,
    texturesFolder: true,
    soundsFolder: true,
    sourceFolder: false,
    gitignore: false,
    ignoreSourceArt: false,
    readme: false,
    credits: false,
    ...extra,
  };
}
