import type {
  ApplyReportDto,
  CeSuggestionDto,
  DesignerApplyPlanRequest,
  DesignerExportPlanRequest,
  DraftDto,
  DraftEntryDto,
  WritePlanDto,
} from 'rimstudio-ipc-types';
import { createEditorStore } from './editor-store';
import { createOutputStore } from './output-store';
import { fixture, installTransport, manualScheduler, settle } from './testSupport';

/** A mock handler: the request in, the response out. */
export type Handler = (request: unknown) => unknown | Promise<unknown>;

/** A stored entry of a recorded draft: the clone of the bolt-action rifle, vanilla or with Combat Extended. */
export function outputEntry(
  name: 'vanilla' | 'ce-on' | 'ce-answered' | 'ce-edited' = 'vanilla',
): DraftEntryDto {
  const draft = fixture<DraftDto>(`designer-output-draft-${name}`);
  return {
    id: 'd-out',
    defName: draft.spec.identity.defName,
    label: draft.spec.identity.label,
    kind: 'ranged',
    updatedAtMs: 1,
    draft,
  };
}

/**
 * The plan the real bridge gave for the same request: the recorded plans are picked by what the
 * request holds (switch, accepted values, answered caliber), so a test runs the whole flow.
 */
export function recordedPlan(request: DesignerExportPlanRequest): WritePlanDto {
  const ce = request.draft.spec.ce;
  if (!ce) return fixture('designer-output-plan-vanilla');
  if (!request.acceptSuggestions) return fixture('designer-output-plan-ce-nothing-accepted');
  if (!ce.ammoSet) return fixture('designer-output-plan-ce-needs-answer');
  return request.draft.spec.ranged?.damage?.value === 21
    ? fixture('designer-output-plan-ce-update')
    : fixture('designer-output-plan-ce-ready');
}

/** The suggestion the real bridge gave: off, on without answers, or on with the caliber answered. */
export function recordedSuggestion(draft: DraftDto): CeSuggestionDto {
  const ce = draft.spec.ce;
  if (!ce) return fixture('designer-output-suggest-off');
  return ce.ammoSet
    ? fixture('designer-output-suggest-answered')
    : fixture('designer-output-suggest-on');
}

export interface OutputSetup {
  transport: ReturnType<typeof installTransport>;
  editor: ReturnType<typeof createEditorStore>;
  output: ReturnType<typeof createOutputStore>;
  /** The timers of the output store. */
  clock: ReturnType<typeof manualScheduler>;
  /** Run the pending timers of the output store and let the answers arrive. */
  flush: () => Promise<void>;
}

/** An editor and an output store on a mock transport that answers with the recorded responses. */
export function setupOutput(
  handlers: Record<string, Handler> = {},
  projectId: () => string | undefined = () => 'p-out',
): OutputSetup {
  const transport = installTransport({
    designer_export_plan: (r) => recordedPlan(r as DesignerExportPlanRequest),
    designer_ce_suggest: (r) => recordedSuggestion((r as { draft: DraftDto }).draft),
    designer_apply_plan: (r) => {
      const request = r as DesignerApplyPlanRequest;
      const name = request.request.draft.spec.ranged?.damage?.value === 21 ? 'updated' : 'created';
      return fixture<ApplyReportDto>(`designer-output-apply-${name}`);
    },
    ...handlers,
  });
  const editorClock = manualScheduler();
  const editor = createEditorStore({ projectId, scheduler: editorClock.scheduler });
  const clock = manualScheduler();
  const output = createOutputStore({ editor, projectId, scheduler: clock.scheduler });
  const flush = async (): Promise<void> => {
    clock.runTimers();
    await settle();
  };
  return { transport, editor, output, clock, flush };
}

/** The number of calls of one command. */
export function callsOf(
  transport: { calls: ReadonlyArray<{ name: string }> },
  name: string,
): number {
  return transport.calls.filter((c) => c.name === name).length;
}
