import { signal } from '@preact/signals';
import { setTransport } from './client';
import { startJobStream, setJobCommands } from './jobs';
import { createHttpTransport } from './transports/http';
import { createMockTransport } from './transports/mock';
import { createTauriTransport } from './transports/tauri';
import type { DevCommandRow, Transport } from './types';

export type ConnectionState = 'connecting' | 'bridge' | 'mock' | 'tauri' | 'lost';

/** What the shell shows in the connection chip. */
export const connection = signal<ConnectionState>('connecting');

let stopStream: (() => void) | undefined;

async function healthy(transport: Transport, timeoutMs: number): Promise<boolean> {
  const timeout = new Promise<boolean>((resolve) => setTimeout(() => resolve(false), timeoutMs));
  const probe = transport.dev('/dev/health').then(
    () => true,
    () => false,
  );
  return Promise.race([probe, timeout]);
}

/**
 * Choose the transport at boot: the Tauri runtime when present, else the bridge when it answers
 * /dev/health, else the fixture backed mock. Returns the transport kind that was installed.
 */
export async function connect(
  options: { forceMock?: boolean; timeoutMs?: number } = {},
): Promise<ConnectionState> {
  stopStream?.();
  stopStream = undefined;
  connection.value = 'connecting';

  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    setTransport(createTauriTransport());
    connection.value = 'tauri';
    return 'tauri';
  }

  const http = createHttpTransport();
  if (!options.forceMock && (await healthy(http, options.timeoutMs ?? 1500))) {
    setTransport(http);
    connection.value = 'bridge';
    stopStream = startJobStream(http);
    try {
      const rows = (await http.dev('/dev/commands')) as DevCommandRow[];
      setJobCommands(rows.filter((r) => r.kind === 'job').map((r) => r.name));
    } catch {
      /* without the listing jobs still show up from the event stream */
    }
    return 'bridge';
  }

  const mock = await createMockTransport();
  setTransport(mock);
  connection.value = 'mock';
  stopStream = startJobStream(mock);
  return 'mock';
}
