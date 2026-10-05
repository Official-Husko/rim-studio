import { signal } from '@preact/signals';
import type {
  ApiError,
  ConvertCandidateDto,
  ConvertRequestDto,
  WritePlanDto,
} from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import type { ProjectRef } from '~/shared/project';
import { planConversion } from './api';
import { familyAnswers, ownAnswers } from './answerStore';
import { buildRequest, familyOf } from './model';
import { withOpenProject } from './session';

/** The plan of one weapon for one request. */
export interface PlanEntry {
  /** The request the plan was made for, as text; a changed answer makes the entry stale. */
  key: string;
  phase: 'loading' | 'ready' | 'error';
  plan?: WritePlanDto;
  error?: ApiError;
}

/** The plans by definition name. */
export const plans = signal<Readonly<Record<string, PlanEntry>>>({});

/** The request that the current answers give for a weapon. */
export function requestFor(candidate: ConvertCandidateDto): ConvertRequestDto {
  return buildRequest(
    candidate,
    ownAnswers.peek()[candidate.defName],
    familyOf(candidate) ? familyAnswers.peek()[familyOf(candidate)] : undefined,
  );
}

function put(defName: string, entry: PlanEntry): void {
  plans.value = { ...plans.peek(), [defName]: entry };
}

/**
 * Plan one weapon with the current answers. A plan that is already there for the same request is reused
 * unless `force` is set; a newer request for the same weapon wins over an older one still running.
 */
export async function loadPlan(
  project: ProjectRef,
  candidate: ConvertCandidateDto,
  force = false,
): Promise<PlanEntry> {
  const request = requestFor(candidate);
  const key = JSON.stringify(request);
  const known = plans.peek()[candidate.defName];
  if (!force && known?.key === key && known.phase !== 'error') return known;
  put(candidate.defName, { key, phase: 'loading' });
  try {
    const plan = await withOpenProject(project, (id) =>
      planConversion(id, candidate.kind ?? 'ranged', request),
    );
    const entry: PlanEntry = { key, phase: 'ready', plan };
    if (plans.peek()[candidate.defName]?.key === key) put(candidate.defName, entry);
    return entry;
  } catch (thrown) {
    const entry: PlanEntry = { key, phase: 'error', error: normalizeError(thrown) };
    if (plans.peek()[candidate.defName]?.key === key) put(candidate.defName, entry);
    return entry;
  }
}

/** Forget every plan (the project or the scan changed). */
export function resetPlans(): void {
  plans.value = {};
}
