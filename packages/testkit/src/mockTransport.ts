import { loadFixture, hasFixture } from './fixtures';

/** The ApiError envelope body of the bridge API (code, message, errorId, details). */
export interface MockApiError {
  code: string;
  message: string;
  errorId: string;
  details?: Record<string, unknown>;
}

/** Events of the bridge SSE stream (GET /dev/events). */
export type MockBridgeEvent =
  | {
      type: 'job-progress';
      jobId: string;
      command: string;
      message: string;
      done: number;
      total: number | null;
    }
  | { type: 'job-finished'; jobId: string; command: string; ok: boolean };

export type MockHandler = (request: unknown) => unknown | Promise<unknown>;

export interface MockTransportOptions {
  /** Command name to handler; wins over fixtures. */
  handlers?: Record<string, MockHandler>;
  /** Command name to a fixed response; falls back to fixtures/<name>.json. */
  responses?: Record<string, unknown>;
  /** Milliseconds of artificial latency (default 0). */
  latencyMs?: number;
}

/** Structurally the same as the Transport interface of the desktop app (shared/ipc). */
export interface MockTransport {
  readonly kind: 'mock';
  call(name: string, request: unknown): Promise<unknown>;
  dev(path: string): Promise<unknown>;
  events(handler: (event: MockBridgeEvent) => void): () => void;
  /** Push an event to every subscriber (tests and the gallery). */
  emit(event: MockBridgeEvent): void;
  /** Every call seen, in order. */
  readonly calls: ReadonlyArray<{ name: string; request: unknown }>;
}

/** Build an error object the client turns into an ApiError. */
export function mockError(
  code: string,
  message: string,
  details?: Record<string, unknown>,
): MockApiError {
  const error: MockApiError = { code, message, errorId: 'e-mock0001' };
  if (details) error.details = details;
  return error;
}

const wait = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

/** A transport that serves handlers, then explicit responses, then fixtures by command name. */
export function createMockTransport(options: MockTransportOptions = {}): MockTransport {
  const subscribers = new Set<(event: MockBridgeEvent) => void>();
  const calls: Array<{ name: string; request: unknown }> = [];
  const latency = options.latencyMs ?? 0;

  return {
    kind: 'mock',
    calls,
    async call(name, request) {
      calls.push({ name, request });
      if (latency > 0) await wait(latency);
      const handler = options.handlers?.[name];
      if (handler) return handler(request);
      if (options.responses && name in options.responses) return options.responses[name];
      if (hasFixture(name)) return loadFixture(name);
      throw mockError('ipc.unknown-command', `The mock transport has no response for ${name}.`);
    },
    async dev(path) {
      if (latency > 0) await wait(latency);
      const url = new URL(path, 'http://mock.invalid');
      switch (url.pathname) {
        case '/dev/health':
          return { ok: true };
        case '/dev/info':
          return loadFixture('dev-info');
        case '/dev/commands':
          return hasFixture('dev-commands') ? loadFixture('dev-commands') : [];
        case '/dev/fs/home':
          return loadFixture('dev-fs-home');
        case '/dev/fs/list': {
          const wanted = url.searchParams.get('path') ?? '/';
          const table = loadFixture('dev-fs-list') as Record<string, unknown>;
          const hit = table[wanted];
          if (hit === undefined) throw mockError('io.not-a-directory', `Not a folder: ${wanted}`);
          return hit;
        }
        default:
          throw mockError(
            'ipc.unknown-command',
            `The mock transport has no route ${url.pathname}.`,
          );
      }
    },
    events(handler) {
      subscribers.add(handler);
      return () => {
        subscribers.delete(handler);
      };
    },
    emit(event) {
      for (const handler of [...subscribers]) handler(event);
    },
  };
}
