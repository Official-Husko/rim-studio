import { callCommand } from '~/shared/ipc';
import type {
  ApplyReportDto,
  CeSuggestionDto,
  DesignerExportPlanRequest,
  DraftDto,
  WritePlanDto,
} from 'rimstudio-ipc-types';

// One function per command used by the output panel; no state and no rules.

/** The files, the diagnostics and the plan id of a draft, without writing anything. */
export function exportPlan(request: DesignerExportPlanRequest): Promise<WritePlanDto> {
  return callCommand('designer_export_plan', request);
}

/** The Combat Extended suggestion of a draft; it works with the switch off too. */
export function ceSuggest(draft: DraftDto): Promise<CeSuggestionDto> {
  return callCommand('designer_ce_suggest', { draft });
}

/** What the user chose in the confirmation dialog. */
export interface ApplyOptions {
  /** Copy a replaced file to the backups folder before it is written. */
  backup: boolean;
  /** Run the written Combat Extended patch against the definitions afterwards. */
  dryApply: boolean;
}

/**
 * Write a reviewed plan (a job: it shows in the task centre). The request is the one that made the
 * plan; the backend rebuilds the plan and refuses it when it differs from the reviewed one.
 */
export function applyPlan(
  planId: string,
  request: DesignerExportPlanRequest,
  options: ApplyOptions,
): Promise<ApplyReportDto> {
  return callCommand('designer_apply_plan', {
    planId,
    request,
    backup: options.backup,
    dryApply: options.dryApply,
  });
}
