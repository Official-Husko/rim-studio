import type {
  ApplyReportDto,
  CeSuggestionDto,
  ConvertRequestDto,
  ConvertScanDto,
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
 * The ranked caliber and weapon class choices for a family of weapons. The scan carries no tags beyond
 * the family key, so the request holds the kind and the first tag only.
 */
export function suggestChoices(
  kind: ItemKindDto,
  defName: string,
  firstTag: string,
): Promise<CeSuggestionDto> {
  const draft = placeholderDraft(kind, defName);
  draft.spec.weaponTags = firstTag ? [firstTag] : [];
  return callCommand('designer_ce_suggest', { draft });
}

/** The text of one project file. */
export function readProjectFile(projectId: string, path: string): Promise<ProjectFileDto> {
  return callCommand('project_read_file', { projectId, path, maxBytes: 200_000 });
}
