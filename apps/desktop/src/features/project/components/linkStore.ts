import { signal } from '@preact/signals';
import type {
  ApiError,
  ProjectLinkModeDto,
  ProjectLinkRefusalDto,
  ProjectLinkResultDto,
  ProjectLinkStatusDto,
} from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { createLink, readLinkStatus, removeLink } from './linkApi';

/** What the link card is doing right now. */
export type LinkBusy = 'idle' | 'loading' | 'creating' | 'removing';

/** What the last create or remove did, shown until the next action. */
export interface LinkNotice {
  kind: 'created' | 'removed' | 'refused';
  refusal?: ProjectLinkRefusalDto;
}

export const linkStatus = signal<ProjectLinkStatusDto | undefined>(undefined);
export const linkBusy = signal<LinkBusy>('idle');
export const linkError = signal<ApiError | undefined>(undefined);
export const linkNotice = signal<LinkNotice | undefined>(undefined);
export const linkConfirming = signal(false);

let token = 0;

/** Forget everything about the previous project. */
export function resetLinkStore(): void {
  token += 1;
  linkStatus.value = undefined;
  linkBusy.value = 'idle';
  linkError.value = undefined;
  linkNotice.value = undefined;
  linkConfirming.value = false;
}

/** Read the status of a project. A stale answer (the project changed meanwhile) is dropped. */
export async function loadLinkStatus(projectId: string, keepNotice = false): Promise<void> {
  token += 1;
  const mine = token;
  if (linkStatus.peek()?.projectId !== projectId) linkStatus.value = undefined;
  if (!keepNotice) linkNotice.value = undefined;
  linkBusy.value = 'loading';
  try {
    const status = await readLinkStatus(projectId);
    if (mine !== token) return;
    linkStatus.value = status;
    linkError.value = undefined;
  } catch (thrown) {
    if (mine !== token) return;
    linkError.value = normalizeError(thrown);
  } finally {
    if (mine === token) linkBusy.value = 'idle';
  }
}

function settle(result: ProjectLinkResultDto, done: LinkNotice['kind']): void {
  linkStatus.value = result.status;
  linkError.value = undefined;
  linkNotice.value = result.done
    ? { kind: done }
    : { kind: 'refused', ...(result.refusal ? { refusal: result.refusal } : {}) };
}

async function act(
  busy: 'creating' | 'removing',
  done: LinkNotice['kind'],
  run: () => Promise<ProjectLinkResultDto>,
): Promise<void> {
  token += 1;
  const mine = token;
  linkBusy.value = busy;
  linkNotice.value = undefined;
  try {
    const result = await run();
    if (mine === token) settle(result, done);
  } catch (thrown) {
    if (mine === token) linkError.value = normalizeError(thrown);
  } finally {
    if (mine === token) linkBusy.value = 'idle';
  }
}

/** Open the confirmation of the link. */
export function askToLink(): void {
  linkConfirming.value = true;
}

/** Close the confirmation. */
export function cancelLink(): void {
  linkConfirming.value = false;
}

/** Create the link after the person confirmed. */
export async function confirmLink(
  projectId: string,
  mode: ProjectLinkModeDto,
  confirmGameRunning: boolean,
): Promise<void> {
  linkConfirming.value = false;
  await act('creating', 'created', () => createLink(projectId, mode, confirmGameRunning));
}

/** Remove the link RimStudio made. */
export async function unlink(projectId: string): Promise<void> {
  await act('removing', 'removed', () => removeLink(projectId));
}
