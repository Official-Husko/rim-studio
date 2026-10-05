import type { ApiError } from 'rimstudio-ipc-types';

/** True for an object shaped like the error envelope. */
export function isApiError(value: unknown): value is ApiError {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as Record<string, unknown>;
  return typeof v.code === 'string' && typeof v.message === 'string';
}

let counter = 0;

/** Turn anything thrown into an ApiError; the UI never matches on message text, only on code. */
export function normalizeError(thrown: unknown): ApiError {
  if (isApiError(thrown)) {
    const v = thrown as ApiError;
    return {
      code: v.code,
      message: v.message,
      errorId: typeof v.errorId === 'string' ? v.errorId : clientErrorId(),
      ...(v.details ? { details: v.details } : {}),
    };
  }
  if (thrown instanceof DOMException && thrown.name === 'AbortError') {
    return { code: 'ipc.aborted', message: 'The call was cancelled.', errorId: clientErrorId() };
  }
  const message = thrown instanceof Error ? thrown.message : String(thrown);
  return { code: 'ipc.transport', message, errorId: clientErrorId() };
}

/** A short id for errors raised in the webview itself (the backend mints its own). */
export function clientErrorId(): string {
  counter += 1;
  return `c-${Date.now().toString(36)}${counter.toString(36)}`;
}
