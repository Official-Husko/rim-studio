/** The error envelope of every rejected call (IPC and state architecture, section 5). */
export type { ApiError } from 'rimstudio-ipc-types';

/** Events of the bridge SSE stream (GET /dev/events). */
export type BridgeEvent =
  | {
      type: 'job-progress';
      jobId: string;
      command: string;
      message: string;
      done: number;
      total: number | null;
    }
  | { type: 'job-finished'; jobId: string; command: string; ok: boolean };

/** What a transport must provide. The http, tauri and mock transports implement it. */
export interface Transport {
  readonly kind: 'http' | 'tauri' | 'mock';
  /** Resolves with the response data or rejects with an ApiError shaped object. */
  call(name: string, request: unknown, signal?: AbortSignal): Promise<unknown>;
  /** GET a development route such as /dev/info and resolve with its JSON. */
  dev(path: string): Promise<unknown>;
  /** Subscribe to job events; the returned function unsubscribes. */
  events(handler: (event: BridgeEvent) => void): () => void;
}

/** The registry row listing of GET /dev/commands. */
export interface DevCommandRow {
  name: string;
  kind: 'query' | 'action' | 'job' | 'stream';
  request: string;
  response: string;
}

export interface DevInfo {
  bridgeVersion: string;
  platform: string;
  home: string;
  dataDir: string;
  commandCount: number;
  /** Hash of the contract the bridge was built with; compare with the bindings. */
  contractHash?: string;
}

export interface DevFsEntry {
  name: string;
  kind: 'dir' | 'file';
  isModFolder: boolean;
  hasAbout: boolean;
}

export interface DevFsListing {
  path: string;
  parent: string | null;
  entries: DevFsEntry[];
}

export interface DevFsHome {
  home: string;
  places: Array<{ label: string; path: string }>;
}
