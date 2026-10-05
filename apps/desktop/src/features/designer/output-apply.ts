import { batch, signal, type ReadonlySignal } from '@preact/signals';
import type {
  ApiError,
  ApplyReportDto,
  DesignerExportPlanRequest,
  WritePlanDto,
} from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { bumpProjectRevision } from '~/shared/project';
import * as api from './output-api';

/** Where the confirmation dialog stands. */
export type ApplyPhase = 'idle' | 'confirm' | 'running' | 'done' | 'failed';

export interface ApplyState {
  phase: ApplyPhase;
  report?: ApplyReportDto;
  error?: ApiError;
}

export interface ApplyFlowDeps {
  /** The reviewed plan. */
  plan: ReadonlySignal<WritePlanDto | undefined>;
  /** The request that made the reviewed plan. */
  request: () => DesignerExportPlanRequest | undefined;
  /** True when the plan may be written: no errors, up to date and something to write. */
  canApply: () => boolean;
  /** Called after a write, successful or not, so the plan is asked for again. */
  after: () => void;
}

/**
 * The confirmation, the write and its result. Nothing is written without the dialog being opened
 * first, and a write is refused while the plan has errors, is out of date or has nothing to write.
 */
export function createApplyFlow(deps: ApplyFlowDeps) {
  const apply = signal<ApplyState>({ phase: 'idle' });
  const lastReport = signal<ApplyReportDto | undefined>(undefined);

  /** Open the confirmation dialog when Apply is possible. */
  function open(): void {
    if (deps.canApply()) apply.value = { phase: 'confirm' };
  }

  /** Close the dialog; a running write cannot be closed. */
  function close(): void {
    if (apply.peek().phase !== 'running') apply.value = { phase: 'idle' };
  }

  /** Open the dialog on the result of the last write. */
  function showReport(): void {
    const report = lastReport.peek();
    if (report && apply.peek().phase === 'idle') apply.value = { phase: 'done', report };
  }

  /** Write the reviewed plan with the choices of the dialog. */
  async function confirm(options: api.ApplyOptions): Promise<void> {
    const current = deps.plan.peek();
    const request = deps.request();
    if (!current || !request || !deps.canApply()) return;
    apply.value = { phase: 'running' };
    try {
      const report = await api.applyPlan(current.planId, request, options);
      batch(() => {
        apply.value = { phase: 'done', report };
        lastReport.value = report;
      });
      bumpProjectRevision();
    } catch (thrown) {
      apply.value = { phase: 'failed', error: normalizeError(thrown) };
    }
    deps.after();
  }

  /** Forget the dialog and the last result (another draft opened). */
  function reset(): void {
    batch(() => {
      apply.value = { phase: 'idle' };
      lastReport.value = undefined;
    });
  }

  return { apply, lastReport, open, close, showReport, confirm, reset };
}

export type ApplyFlow = ReturnType<typeof createApplyFlow>;
