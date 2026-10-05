import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { clientErrorId } from '../error';
import type { ApiError, BridgeEvent, Transport } from '../types';

/** The Tauri event the shell sends for every job event (same JSON as the bridge's /dev/events). */
export const JOB_EVENT = 'rs-job';

function unavailable(path: string): ApiError {
  return {
    code: 'platform.unavailable',
    message: `${path} is a browser only route: the desktop app uses native dialogs.`,
    errorId: clientErrorId(),
  };
}

/** Reject when the signal aborts, like fetch does, so a caller can stop waiting. */
function abortable<T>(work: Promise<T>, signal: AbortSignal | undefined): Promise<T> {
  if (!signal) return work;
  if (signal.aborted) return Promise.reject(new DOMException('Aborted', 'AbortError'));
  return new Promise<T>((resolve, reject) => {
    const onAbort = (): void => reject(new DOMException('Aborted', 'AbortError'));
    signal.addEventListener('abort', onAbort, { once: true });
    work.then(
      (value) => {
        signal.removeEventListener('abort', onAbort);
        resolve(value);
      },
      (error: unknown) => {
        signal.removeEventListener('abort', onAbort);
        reject(error);
      },
    );
  });
}

/**
 * The desktop transport: commands go to the Rust shell through `invoke` and job events arrive as
 * Tauri events. A rejected call carries the ApiError envelope of the registry unchanged.
 */
export function createTauriTransport(): Transport {
  return {
    kind: 'tauri',
    call(name, request, signal) {
      return abortable(invoke('rs_call', { name, request: request ?? {} }), signal);
    },
    dev(path) {
      const route = path.split('?')[0];
      switch (route) {
        case '/dev/health':
          return Promise.resolve({ ok: true });
        case '/dev/info':
          return invoke('rs_info');
        case '/dev/commands':
          return invoke('rs_commands');
        default:
          return Promise.reject(unavailable(route ?? path));
      }
    },
    events(handler) {
      let stopped = false;
      let unlisten: (() => void) | undefined;
      void listen<BridgeEvent>(JOB_EVENT, (event) => handler(event.payload)).then(
        (off) => {
          if (stopped) off();
          else unlisten = off;
        },
        () => undefined,
      );
      return () => {
        stopped = true;
        unlisten?.();
      };
    },
  };
}
