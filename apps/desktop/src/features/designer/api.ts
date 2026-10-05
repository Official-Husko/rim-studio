import { callCommand } from '~/shared/ipc';
import type {
  AnchorDto,
  CalibrateResultDto,
  DesignerCloneDiffResponse,
  DesignerCloneResponse,
  DesignerDraftListResponse,
  DesignerDraftSaveResponse,
  DesignerProjectileOwnResponse,
  DesignerQuizAnswerResponse,
  DesignerStructureDefaultsResponse,
  DraftDto,
  FitReportDto,
  ItemKindDto,
  PreviewDto,
  ProjectSummaryDto,
  QuizAnswerDto,
  QuizStepDto,
  ReferenceListDto,
  ResolvedDefDto,
  TechLevelDto,
} from 'rimstudio-ipc-types';

// One function per command; no state and no rules. The stores call these.

/** The session that holds the game and its expansions: the reference set of the designer. */
const REFERENCE_SESSION = 'reference';

/** Everything the reference browser can show for one kind, with the optional backend filters. */
export function referenceList(
  kind: ItemKindDto,
  filters: { role?: string; tier?: TechLevelDto } = {},
): Promise<ReferenceListDto> {
  return callCommand('designer_reference_list', { kind, offset: 0, limit: 500, ...filters });
}

/** The exact readouts, the suggestions and the diagnostics of a draft. */
export function preview(draft: DraftDto): Promise<PreviewDto> {
  return callCommand('designer_preview', { draft });
}

/** The fit of the typed values against the pool bands. */
export function fit(draft: DraftDto): Promise<FitReportDto> {
  return callCommand('designer_fit', { draft });
}

/** The first or the next question of the estimate dialogue. */
export function quizNext(draft: DraftDto): Promise<QuizStepDto> {
  return callCommand('designer_quiz_next', { draft });
}

/** Record an answer; the response holds the draft with the answer and the next step. */
export function quizAnswer(
  draft: DraftDto,
  questionId: string,
  answer: QuizAnswerDto,
): Promise<DesignerQuizAnswerResponse> {
  return callCommand('designer_quiz_answer', { draft, questionId, answer });
}

/** Take back the last answer. */
export function quizBack(draft: DraftDto): Promise<DesignerQuizAnswerResponse> {
  return callCommand('designer_quiz_back', { draft });
}

/** Store a draft; without an id a new one is stored. */
export function draftSave(
  projectId: string,
  draft: DraftDto,
  id?: string,
): Promise<DesignerDraftSaveResponse> {
  return callCommand(
    'designer_draft_save',
    id === undefined ? { projectId, draft } : { projectId, draft, id },
  );
}

/** The drafts of a project, newest first as the backend orders them. */
export function draftList(projectId: string): Promise<DesignerDraftListResponse> {
  return callCommand('designer_draft_list', { projectId });
}

/** Delete a stored draft. */
export async function draftDelete(projectId: string, id: string): Promise<boolean> {
  return (await callCommand('designer_draft_delete', { projectId, id })).deleted;
}

/** Clone a loaded weapon into a new stored draft. */
export function cloneWeapon(
  projectId: string,
  source: string,
  defName: string,
  label?: string,
  ownProjectile?: boolean,
): Promise<DesignerCloneResponse> {
  return callCommand('designer_clone', {
    projectId,
    source,
    defName,
    ...(label === undefined ? {} : { label }),
    ...(ownProjectile === undefined ? {} : { ownProjectile }),
  });
}

/** Give a gun draft a projectile of its own, or point it back at the shared one. */
export function projectileOwn(
  draft: DraftDto,
  own: boolean,
): Promise<DesignerProjectileOwnResponse> {
  return callCommand('designer_projectile_own', { draft, own });
}

/** The changed fields and the readout deltas of a clone against its source. */
export function cloneDiff(draft: DraftDto): Promise<DesignerCloneDiffResponse> {
  return callCommand('designer_clone_diff', { draft });
}

/** Suggested parent, projectile, cost list and stuff of a new weapon. */
export function structureDefaults(draft: DraftDto): Promise<DesignerStructureDefaultsResponse> {
  return callCommand('designer_structure_defaults', { draft });
}

/** Calibrate the bands of a kind (a job: it shows in the task centre). */
export function calibrate(kind: ItemKindDto, force: boolean): Promise<CalibrateResultDto> {
  return callCommand('designer_calibrate', { kind, force });
}

/** The resolved definition of a loaded item, as the node tree of the engine. */
export function resolveDefinition(defName: string): Promise<ResolvedDefDto> {
  return callCommand('defs_get_resolved', {
    sessionId: REFERENCE_SESSION,
    defType: 'ThingDef',
    defName,
  });
}

/** Open a project folder (used only while no shared project selector exists). */
export function openProject(path: string): Promise<ProjectSummaryDto> {
  return callCommand('project_open', { path });
}

/** The anchor entry of a reference item. */
export function anchorOf(defName: string, label: string): AnchorDto {
  return { defName, label };
}
