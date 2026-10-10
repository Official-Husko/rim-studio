import type {
  ApplyReportDto,
  CePatchSpecDto,
  CeSuggestionDto,
  ConvertRequestDto,
  ConvertScanDto,
  DesignerLintFilesResult,
  DraftDto,
  ItemKindDto,
  ProjectFileDto,
  WritePlanDto,
} from 'rimstudio-ipc-types';
import { callCommand } from '~/shared/ipc';

// One function per command; no state, no rules. The stores call these.

/**
 * A draft the export request needs but a conversion never reads: the plan of a conversion is built from
 * the definitions of the project and the answers, not from the draft.
 */
function placeholderDraft(kind: ItemKindDto, defName: string): DraftDto {
  return {
    schemaVersion: 1,
    kind,
    calibration: 'simple',
    spec: {
      kind,
      identity: { defName, label: '', description: '', modPrefix: '' },
      previewQuality: 'normal',
      ...(kind === 'ranged' ? { ranged: { accuracy: {}, verbClass: 'Verb_Shoot' } } : {}),
    },
  };
}

/** The convert scan of a project. A job: it shows in the task centre. */
export function scanProject(projectId: string, includeConverted: boolean): Promise<ConvertScanDto> {
  return callCommand('designer_convert_scan', { projectId, includeConverted });
}

/** The write plan of one conversion; reads the project and writes nothing. */
export function planConversion(
  projectId: string,
  kind: ItemKindDto,
  convert: ConvertRequestDto,
): Promise<WritePlanDto> {
  return callCommand('designer_export_plan', {
    projectId,
    draft: placeholderDraft(kind, convert.defName),
    convert,
  });
}

/** Apply a reviewed plan. A job: the plan is rebuilt from the same request and must match its id. */
export function applyConversion(
  planId: string,
  projectId: string,
  kind: ItemKindDto,
  convert: ConvertRequestDto,
  options: { backup: boolean; dryApply: boolean },
): Promise<ApplyReportDto> {
  return callCommand('designer_apply_plan', {
    planId,
    request: { projectId, draft: placeholderDraft(kind, convert.defName), convert },
    backup: options.backup,
    dryApply: options.dryApply,
  });
}

/**
 * The request that takes the generated patch of one weapon out of the project. The backend reads no answer
 * for a removal, so the answers are the empty ones (the two flags are required by the type).
 */
function removalRequest(defName: string): ConvertRequestDto {
  return {
    defName,
    answers: { overrides: { oneHanded: false, beltFed: false } },
    remove: true,
  };
}

/**
 * The write plan that removes the generated patch of one weapon: the files that lose its sections, and
 * the files that go because nothing else is in them. Reads the project and writes nothing.
 */
export function planRemoval(
  projectId: string,
  kind: ItemKindDto,
  defName: string,
): Promise<WritePlanDto> {
  return planConversion(projectId, kind, removalRequest(defName));
}

/**
 * Apply a reviewed removal. A job. The backend keeps a backup of every file it changes whatever the flag
 * says and a removal has no dry apply, so the options are fixed.
 */
export function applyRemoval(
  planId: string,
  projectId: string,
  kind: ItemKindDto,
  defName: string,
): Promise<ApplyReportDto> {
  return applyConversion(planId, projectId, kind, removalRequest(defName), {
    backup: true,
    dryApply: false,
  });
}

/**
 * The ranked caliber and weapon class choices for a weapon, from its kind and the weapon tags of the scan.
 * The request holds no damage, so the ranking is approximate.
 */
export function suggestChoices(
  kind: ItemKindDto,
  defName: string,
  tags: readonly string[],
): Promise<CeSuggestionDto> {
  const draft = placeholderDraft(kind, defName);
  draft.spec.weaponTags = [...tags];
  return callCommand('designer_ce_suggest', { draft });
}

/**
 * The suggestion for a weapon whose block holds the given members (the weapon tag class answered, for
 * example). Only the optional additions it offers are read from the result.
 */
export function suggestWithBlock(
  kind: ItemKindDto,
  defName: string,
  tags: readonly string[],
  ce: CePatchSpecDto,
): Promise<CeSuggestionDto> {
  const draft = placeholderDraft(kind, defName);
  draft.spec.weaponTags = [...tags];
  draft.spec.ce = ce;
  return callCommand('designer_ce_suggest', { draft });
}

/** The lint of every patch file of the project (or of the given files); reads, writes nothing. */
export function lintProjectFiles(
  projectId: string,
  paths?: readonly string[],
): Promise<DesignerLintFilesResult> {
  return callCommand('designer_lint_files', {
    projectId,
    ...(paths && paths.length > 0 ? { paths: [...paths] } : {}),
  });
}

/** The text of one project file. */
export function readProjectFile(projectId: string, path: string): Promise<ProjectFileDto> {
  return callCommand('project_read_file', { projectId, path, maxBytes: 200_000 });
}
