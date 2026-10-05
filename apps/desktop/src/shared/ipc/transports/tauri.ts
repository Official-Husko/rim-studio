import type { ApiError, Transport } from '../types';

const NOT_BUILT: ApiError = {
  code: 'ipc.transport',
  message:
    'The Tauri transport is not available yet: the desktop shell has not been built. Use the bridge or the mock transport.',
  errorId: 'c-tauri-stub',
};

/**
 * Placeholder for the Tauri transport. The shell does not exist yet; the real one will call the
 * generated bindings through shared/platform. It fails with a clear message instead of pretending.
 */
export function createTauriTransport(): Transport {
  return {
    kind: 'tauri',
    call: () => Promise.reject(NOT_BUILT),
    dev: () => Promise.reject(NOT_BUILT),
    events: () => () => {},
  };
}
