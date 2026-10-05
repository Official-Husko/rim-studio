import { signal } from '@preact/signals';
import type {
  ApiError,
  LayoutFixItemDto,
  ProjectLayoutFixApplyDto,
  ProjectLayoutFixPlanDto,
  ProjectLayoutFixUndoDto,
  LayoutFixJournalDto,
} from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { bumpProjectRevision } from '~/shared/project';
import { applyLayoutFix, layoutFixHistory, planLayoutFix, undoLayoutFix } from './api';
import { loadProject, view } from './store';

// The state of the layout fix flow: plan, review, confirm, run, result, and the history with undo.
// The plan, the order, the risk and the conflicts all come from the backend; this file only keeps
// which items the user ticked.

export type FixPhase = 'closed' | 'loading' | 'review' | 'confirm' | 'running' | 'result' | 'error';

export interface FixFlow {
  phase: FixPhase;
  plan?: ProjectLayoutFixPlanDto;
  /** Ids of the ticked items. */
  selected: readonly string[];
  /** Ids of the items that take the numbered name when the destination exists. */
  rename: readonly string[];
  result?: ProjectLayoutFixApplyDto;
  error?: ApiError;
  /** What the dialog was opened for, so Review again plans the same scope. */
  codes?: readonly string[];
  path?: string;
}

const CLOSED: FixFlow = { phase: 'closed', selected: [], rename: [] };

export const fixFlow = signal<FixFlow>(CLOSED);

/** State of an undo, for the result view and the history. */
export interface UndoFlow {
  applyId: string;
  running: boolean;
  result?: ProjectLayoutFixUndoDto;
  error?: ApiError;
}
export const undoFlow = signal<UndoFlow | undefined>(undefined);

export interface HistoryFlow {
  open: boolean;
  loading: boolean;
  journals: readonly LayoutFixJournalDto[];
  error?: ApiError;
}
export const historyFlow = signal<HistoryFlow>({ open: false, loading: false, journals: [] });

let planToken = 0;

function patch(next: Partial<FixFlow>): void {
  fixFlow.value = { ...fixFlow.value, ...next };
}

/** The items a user can tick: the backend says which can be applied. */
export function isSelectable(item: LayoutFixItemDto): boolean {
  return item.applicable;
}

function defaultSelection(plan: ProjectLayoutFixPlanDto, path: string | undefined): string[] {
  const ready = plan.items.filter((i) => isSelectable(i) && !i.conflict?.destinationExists);
  let chosen = ready;
  if (path) {
    const own = ready.filter((i) => i.from === path || i.to === path);
    if (own.length > 0) chosen = own;
  }
  const ids = new Set(chosen.map((i) => i.id));
  for (const item of chosen) for (const need of item.requires) ids.add(need);
  return plan.items.filter((i) => ids.has(i.id)).map((i) => i.id);
}

/** Plan the fixes (all of them, or the ones of some issue codes) and open the review. */
export async function openFix(
  options: { codes?: readonly string[]; path?: string } = {},
): Promise<void> {
  const current = view.peek();
  if (!current) return;
  planToken += 1;
  const token = planToken;
  undoFlow.value = undefined;
  fixFlow.value = {
    phase: 'loading',
    selected: [],
    rename: [],
    ...(options.codes ? { codes: options.codes } : {}),
    ...(options.path ? { path: options.path } : {}),
  };
  try {
    const plan = await planLayoutFix(current.summary.projectId, options.codes);
    if (token !== planToken) return;
    patch({ phase: 'review', plan, selected: defaultSelection(plan, options.path), rename: [] });
  } catch (thrown) {
    if (token !== planToken) return;
    patch({ phase: 'error', error: normalizeError(thrown) });
  }
}

/** Plan again with the scope the dialog was opened with (after a stale plan). */
export async function reviewAgain(): Promise<void> {
  const flow = fixFlow.value;
  await openFix({
    ...(flow.codes ? { codes: flow.codes } : {}),
    ...(flow.path ? { path: flow.path } : {}),
  });
}

/** Tick or untick an item. A ticked item brings the items it requires; an unticked one drops its dependants. */
export function toggleItem(id: string, on: boolean): void {
  const flow = fixFlow.value;
  const plan = flow.plan;
  if (!plan) return;
  const item = plan.items.find((i) => i.id === id);
  if (!item || !isSelectable(item)) return;
  const selected = new Set(flow.selected);
  if (on) {
    selected.add(id);
    for (const need of item.requires) selected.add(need);
  } else {
    selected.delete(id);
    for (const other of plan.items) {
      if (other.requires.includes(id)) selected.delete(other.id);
    }
  }
  patch({ selected: plan.items.filter((i) => selected.has(i.id)).map((i) => i.id) });
}

/** Choose the numbered name for an item whose destination exists; this also ticks the item. */
export function toggleRename(id: string, on: boolean): void {
  const flow = fixFlow.value;
  const rename = new Set(flow.rename);
  if (on) rename.add(id);
  else rename.delete(id);
  patch({ rename: [...rename] });
  if (on) toggleItem(id, true);
  else if (flow.selected.includes(id)) toggleItem(id, false);
}

/** Tick every item that can be applied without a conflict. */
export function selectAllSafe(): void {
  const plan = fixFlow.value.plan;
  if (plan) patch({ selected: defaultSelection(plan, undefined) });
}

/** Clear the ticks. */
export function selectNone(): void {
  patch({ selected: [] });
}

/** Go to the confirmation. */
export function goConfirm(): void {
  if (fixFlow.value.selected.length > 0) patch({ phase: 'confirm' });
}

/** Back from the confirmation to the review. */
export function backToReview(): void {
  patch({ phase: 'review' });
}

async function refreshProject(): Promise<void> {
  const current = view.peek();
  if (!current) return;
  bumpProjectRevision();
  await loadProject({
    projectId: current.summary.projectId,
    path: current.summary.path,
    name: current.summary.name,
  });
}

/** Carry out the ticked items; then show the result and read the project again. */
export async function runApply(): Promise<void> {
  const current = view.peek();
  const flow = fixFlow.value;
  if (!current || !flow.plan || flow.selected.length === 0) return;
  patch({ phase: 'running' });
  try {
    const result = await applyLayoutFix(
      current.summary.projectId,
      flow.plan.planId,
      flow.selected.map((id) => ({ id, renameOnConflict: flow.rename.includes(id) })),
    );
    patch({ phase: 'result', result });
    await refreshProject();
  } catch (thrown) {
    patch({ phase: 'error', error: normalizeError(thrown) });
  }
}

/** Close the dialog. A running apply cannot be closed. */
export function closeFix(): void {
  if (fixFlow.value.phase === 'running') return;
  planToken += 1;
  fixFlow.value = CLOSED;
}

/** Reverse one apply, then read the project and the history again. */
export async function undoApply(applyId: string): Promise<boolean> {
  const current = view.peek();
  if (!current) return false;
  undoFlow.value = { applyId, running: true };
  try {
    const result = await undoLayoutFix(current.summary.projectId, applyId);
    undoFlow.value = { applyId, running: false, result };
    await refreshProject();
    if (historyFlow.peek().open) await loadHistory();
    return true;
  } catch (thrown) {
    undoFlow.value = { applyId, running: false, error: normalizeError(thrown) };
    if (historyFlow.peek().open) await loadHistory();
    return false;
  }
}

/** Read the history of the project. */
export async function loadHistory(): Promise<void> {
  const current = view.peek();
  if (!current) return;
  historyFlow.value = { ...historyFlow.value, loading: true };
  try {
    const { journals } = await layoutFixHistory(current.summary.projectId);
    historyFlow.value = { open: historyFlow.value.open, loading: false, journals };
  } catch (thrown) {
    historyFlow.value = {
      open: historyFlow.value.open,
      loading: false,
      journals: [],
      error: normalizeError(thrown),
    };
  }
}

/** Open the history list. */
export async function openHistory(): Promise<void> {
  undoFlow.value = undefined;
  historyFlow.value = { open: true, loading: true, journals: [] };
  await loadHistory();
}

/** Close the history list. */
export function closeHistory(): void {
  historyFlow.value = { open: false, loading: false, journals: [] };
}

/** Forget everything (tests). */
export function resetFixFlow(): void {
  planToken += 1;
  fixFlow.value = CLOSED;
  undoFlow.value = undefined;
  historyFlow.value = { open: false, loading: false, journals: [] };
}
