import { clientErrorId, isApiError } from '../error';
import type { ApiError, BridgeEvent, Transport } from '../types';

interface Envelope {
  ok: boolean;
  data?: unknown;
  error?: unknown;
}

function isEnvelope(value: unknown): value is Envelope {
  return typeof value === 'object' && value !== null && typeof (value as Envelope).ok === 'boolean';
}

function protocolError(status: number, text: string): ApiError {
  return {
    code: 'ipc.protocol',
    message: `Unexpected HTTP ${status} from the bridge. ${text}`.trim(),
    errorId: clientErrorId(),
  };
}

/**
 * The browser transport: POST /rpc/<command> with the request JSON and read the bridge envelope.
 * The Vite dev server proxies /rpc and /dev to the bridge and adds the token header.
 */
export function createHttpTransport(base = ''): Transport {
  return {
    kind: 'http',
    async call(name, request, signal) {
      const res = await fetch(`${base}/rpc/${encodeURIComponent(name)}`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(request ?? {}),
        signal,
      });
      const text = await res.text();
      let body: unknown;
      try {
        body = JSON.parse(text);
      } catch {
        throw protocolError(res.status, text.slice(0, 120));
      }
      if (isEnvelope(body)) {
        if (body.ok) return body.data;
        if (isApiError(body.error)) throw body.error;
      }
      if (isApiError(body)) throw body;
      throw protocolError(res.status, 'The body is not an envelope.');
    },
    async dev(path) {
      const res = await fetch(`${base}${path}`);
      const text = await res.text();
      let body: unknown;
      try {
        body = JSON.parse(text);
      } catch {
        throw protocolError(res.status, text.slice(0, 120));
      }
      if (!res.ok) {
        const inner = isEnvelope(body) ? body.error : body;
        throw isApiError(inner) ? inner : protocolError(res.status, 'The request failed.');
      }
      return body;
    },
    events(handler) {
      const source = new EventSource(`${base}/dev/events`);
      source.onmessage = (message: MessageEvent<string>) => {
        try {
          const parsed: unknown = JSON.parse(message.data);
          if (typeof parsed === 'object' && parsed !== null && 'type' in parsed)
            handler(parsed as BridgeEvent);
        } catch {
          /* a malformed event is ignored */
        }
      };
      return () => source.close();
    },
  };
}
