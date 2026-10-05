import { signal } from '@preact/signals';
import type { ApiError, ApplyReportDto, ConvertCandidateDto } from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { bumpProjectRevision, type ProjectRef } from '~/shared/project';
import { applyConversion } from './api';
import { loadPlan, plans, requestFor, resetPlans, type PlanEntry } from './planStore';
import { checked } from './scanStore';
import { withOpenProject } from './session';

/** One weapon of the review: its plan, ready or not. */
export interface ReviewItem {
  candidate: ConvertCandidateDto;
  entry: PlanEntry;
}

/** What happened to one weapon. */
export interface ApplyResult {
  defName: string;
  report?: ApplyReportDto;
  error?: ApiError;
  /** Why the weapon was not applied (a plan with errors). */
  skipped?: string;
}

export type ApplyState =
  | { phase: 'closed' }
  | { phase: 'planning' }
  | { phase: 'review'; items: ReviewItem[] }
  | { phase: 'running'; done: number; total: number; current: string }
  | { phase: 'done'; results: ApplyResult[] };

export const applyState = signal<ApplyState>({ phase: 'closed' });

/** Whether a plan can be written: it loaded, has files and no error diagnostic. */
export function isReady(entry: PlanEntry): boolean {
  return (
    entry.phase === 'ready' && !!entry.plan && !entry.plan.hasErrors && entry.plan.files.length > 0
  );
}

/** Plan the weapons and open the review. */
export async function openReview(
  project: ProjectRef,
  selected: ConvertCandidateDto[],
): Promise<void> {
  applyState.value = { phase: 'planning' };
  const items: ReviewItem[] = [];
  for (const candidate of selected) {
    items.push({ candidate, entry: await loadPlan(project, candidate) });
  }
  applyState.value = { phase: 'review', items };
}

/** Close the dialog; a finished run leaves the plans stale, so they are dropped. */
export function closeReview(): void {
  if (applyState.peek().phase === 'done') resetPlans();
  applyState.value = { phase: 'closed' };
}

/**
 * Apply the ready weapons one after the other. Every weapon is planned again right before it is applied,
 * because the weapons of a mod share one patch file and `LoadFolders.xml`, which the weapon before it has
 * just changed.
 */
export async function runApply(
  project: ProjectRef,
  items: ReviewItem[],
  options: { backup: boolean; dryApply: boolean },
): Promise<void> {
  // a second click, or a second call, while a run is going must not start another one
  if (applyState.peek().phase === 'running') return;
  const ready = items.filter((i) => isReady(i.entry));
  const results: ApplyResult[] = items
    .filter((i) => !isReady(i.entry))
    .map((i) => ({ defName: i.candidate.defName, skipped: 'not-ready' }));
  for (const [index, item] of ready.entries()) {
    const defName = item.candidate.defName;
    applyState.value = { phase: 'running', done: index, total: ready.length, current: defName };
    const fresh = await loadPlan(project, item.candidate, true);
    const plan = fresh.plan;
    if (fresh.phase !== 'ready' || !plan || plan.hasErrors) {
      results.push({
        defName,
        skipped: 'plan-failed',
        ...(fresh.error ? { error: fresh.error } : {}),
      });
      continue;
    }
    try {
      const report = await withOpenProject(project, (id) =>
        applyConversion(
          plan.planId,
          id,
          item.candidate.kind ?? 'ranged',
          requestFor(item.candidate),
          options,
        ),
      );
      results.push({ defName, report });
    } catch (thrown) {
      results.push({ defName, error: normalizeError(thrown) });
      break;
    }
  }
  const written = new Set(results.filter((r) => r.report).map((r) => r.defName));
  checked.value = new Set([...checked.peek()].filter((n) => !written.has(n)));
  plans.value = {};
  applyState.value = { phase: 'done', results };
  // the page rescans when the revision changes
  if (written.size > 0) bumpProjectRevision();
}

/** Back to the closed state (tests and project changes). */
export function resetApply(): void {
  applyState.value = { phase: 'closed' };
}
