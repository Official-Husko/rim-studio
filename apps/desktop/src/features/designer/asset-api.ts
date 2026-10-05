import { callCommand } from '~/shared/ipc';
import type { DefPage, DesignerAssetInfoResponse } from 'rimstudio-ipc-types';

// One function per command used by the texture and sound panels; no state and no rules.

/** The session that holds the game and its expansions: the reference set of the designer. */
const REFERENCE_SESSION = 'reference';

/** Rows of one page of the sound search. */
export const SOUND_PAGE = 40;

/** What the backend finds at a file: kind, size, hash, dimensions, thumbnail and problems. */
export function assetInfo(path: string, projectId?: string): Promise<DesignerAssetInfoResponse> {
  return callCommand('designer_asset_info', projectId ? { path, projectId } : { path });
}

/** The sound definitions of the reference set that match a text. */
export function searchSounds(query: string, queryId: string): Promise<DefPage> {
  return callCommand('defs_search', {
    sessionId: REFERENCE_SESSION,
    query,
    defTypes: ['SoundDef'],
    modIds: [],
    hideAbstract: true,
    offset: 0,
    limit: SOUND_PAGE,
    queryId,
  });
}
