import { getTransport } from './client';
import { normalizeError } from './error';
import type { DevCommandRow, DevFsHome, DevFsListing, DevInfo } from './types';

async function get<T>(path: string): Promise<T> {
  try {
    return (await getTransport().dev(path)) as T;
  } catch (thrown) {
    throw normalizeError(thrown);
  }
}

/** GET /dev/info. */
export const devInfo = (): Promise<DevInfo> => get<DevInfo>('/dev/info');

/** GET /dev/commands: every registry row with its kind. */
export const devCommands = (): Promise<DevCommandRow[]> => get<DevCommandRow[]>('/dev/commands');

/** GET /dev/fs/home: the home folder and the places the app knows. */
export const devFsHome = (): Promise<DevFsHome> => get<DevFsHome>('/dev/fs/home');

/** GET /dev/fs/list: the entries of an absolute folder path (the folder browser of the web build). */
export const devFsList = (path: string, includeFiles = false): Promise<DevFsListing> =>
  get<DevFsListing>(
    `/dev/fs/list?path=${encodeURIComponent(path)}${includeFiles ? '&files=1' : ''}`,
  );
