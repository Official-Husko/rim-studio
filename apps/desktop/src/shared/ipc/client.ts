import { signal } from '@preact/signals';
import type { ApiError, CommandName, CommandRequest, CommandResponse } from 'rimstudio-ipc-types';
import { normalizeError } from './error';
import { isJobCommand, trackCall } from './jobs';
import type { Transport } from './types';

/** Which transport serves the app right now; set once at boot by connect(). */
let current: Transport | undefined;

/** Reactive view of the active transport kind, for the connection chip. */
export const transportKind = signal<Transport['kind'] | 'none'>('none');

/** Install the transport every call goes through. */
export function setTransport(transport: Transport): void {
  current = transport;
  transportKind.value = transport.kind;
}

/** The active transport; throws an ApiError when none was installed. */
export function getTransport(): Transport {
  if (!current) {
    const error: ApiError = {
      code: 'ipc.transport',
      message: 'No transport is installed yet.',
      errorId: 'c-no-transport',
    };
    throw error;
  }
  return current;
}

/**
 * Call a registry command by its snake_case name. Resolves with the response DTO; rejects with a
 * normalised ApiError. Job commands are also shown in the task centre while they run.
 */
export async function call<Res = unknown, Req = unknown>(
  name: string,
  request?: Req,
  abort?: AbortSignal,
): Promise<Res> {
  const job = isJobCommand(name) ? trackCall(name) : undefined;
  try {
    const data = await getTransport().call(name, request ?? {}, abort);
    job?.finish(true);
    return data as Res;
  } catch (thrown) {
    job?.finish(false);
    throw normalizeError(thrown);
  }
}

/** The arguments of a typed call: the request is optional when every field of it is optional. */
type CommandArgs<N extends CommandName> = symbol extends keyof CommandRequest<N>
  ? [request?: CommandRequest<N>] // an empty request is typed Record<symbol, never>
  : Partial<CommandRequest<N>> extends CommandRequest<N>
    ? [request?: CommandRequest<N>]
    : [request: CommandRequest<N>];

/**
 * A call checked against the generated command table: the name, the request and the response
 * come from rimstudio-ipc-types. Features wrap this in their api.ts, one function per command.
 */
export function callCommand<N extends CommandName>(
  name: N,
  ...args: CommandArgs<N>
): Promise<CommandResponse<N>> {
  return call<CommandResponse<N>>(name, args[0]);
}
