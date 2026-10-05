import { batch, computed, effect, signal } from '@preact/signals';
import type {
  ApiError,
  CePatchSpecDto,
  CeSuggestionDto,
  DesignerExportPlanRequest,
  DraftDto,
  WritePlanDto,
} from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import type { MessageKey } from '~/shared/i18n';
import type { EditorStore } from './editor-store';
import * as api from './output-api';
import { createApplyFlow } from './output-apply';
import {
  acceptedFor,
  ceIsOn,
  effectiveAccepted,
  emptyCeBlock,
  pendingAnswers,
  setCeValue,
  toggled,
  type AcceptMode,
} from './output-model';
import { browserScheduler, type Scheduler } from './scheduler';
import { setAt, type Pointer } from './model/pointer';

/** Milliseconds of silence after an edit before the plan is asked again. */
export const PLAN_IDLE_MS = 250;

export interface OutputDeps {
  editor: EditorStore;
  /** The id of the current project; undefined when none is open. */
  projectId: () => string | undefined;
  scheduler?: Scheduler;
}

/**
 * The plan of the open draft and the Combat Extended suggestion beside it, kept fresh after every
 * edit. The plan is never applied from here without the dialog: Apply asks for a confirmation and
 * is refused while the plan has errors, is out of date or has nothing to write. Answers of the
 * Combat Extended section go into the draft through the editor (so they are saved with it); the
 * choice of which derived values the plan takes is only a plan parameter and never touches the draft.
 */
export function createOutputStore(deps: OutputDeps) {
  const { editor } = deps;
  const sched = deps.scheduler ?? browserScheduler;

  const plan = signal<WritePlanDto | undefined>(undefined);
  const planError = signal<ApiError | undefined>(undefined);
  const planning = signal(false);
  /** True from an edit until the plan that follows it has arrived. */
  const stale = signal(false);
  const suggestion = signal<CeSuggestionDto | undefined>(undefined);
  const suggestError = signal<ApiError | undefined>(undefined);
  const acceptMode = signal<AcceptMode>('none');
  const customFields = signal<string[]>([]);
  const selectedPath = signal<string | undefined>(undefined);

  /** The request that made the current plan; Apply sends it back so the backend can compare. */
  let planRequest: DesignerExportPlanRequest | undefined;
  const flow = createApplyFlow({
    plan,
    request: () => planRequest,
    canApply: () => canApply.peek(),
    after: () => schedule(false),
  });
  let seq = 0;
  let cancelTimer: (() => void) | undefined;
  const stashes = new Map<string, CePatchSpecDto>();

  const ceOn = computed(() => {
    const draft = editor.draft.value;
    return draft ? ceIsOn(draft.spec) : false;
  });
  const accepted = computed(() =>
    effectiveAccepted(acceptMode.value, customFields.value, suggestion.value),
  );
  const pending = computed(() => pendingAnswers(plan.value));
  const hasChanges = computed(
    () => plan.value?.files.some((f) => f.action !== 'unchanged') ?? false,
  );

  /** Why Apply is not possible right now, as a catalog key; undefined when it is possible. */
  const blockedBy = computed<MessageKey | undefined>(() => {
    if (!editor.draft.value) return 'designer.output.blocked.noDraft';
    if (planError.value) return 'designer.output.blocked.failed';
    if (!plan.value || stale.value || planning.value) return 'designer.output.blocked.planning';
    if (plan.value.hasErrors) return 'designer.output.blocked.errors';
    if (!hasChanges.value) return 'designer.output.blocked.nothing';
    return undefined;
  });
  const canApply = computed(() => blockedBy.value === undefined);

  async function run(refetch: boolean): Promise<void> {
    const draft = editor.draft.peek();
    const projectId = deps.projectId();
    if (!draft || !projectId) {
      batch(() => {
        plan.value = undefined;
        stale.value = false;
        planning.value = false;
      });
      return;
    }
    const mine = ++seq;
    planning.value = true;
    try {
      let current = suggestion.peek();
      const on = ceIsOn(draft.spec);
      // with the switch off the suggestion is asked once, only to learn whether Combat Extended exists
      if (refetch && (on || current === undefined)) {
        try {
          current = await api.ceSuggest(draft);
          if (mine !== seq) return;
          batch(() => {
            suggestion.value = current;
            suggestError.value = undefined;
          });
        } catch (thrown) {
          if (mine !== seq) return;
          suggestError.value = normalizeError(thrown);
        }
      }
      const fields = acceptedFor(on, acceptMode.peek(), customFields.peek(), current);
      const request: DesignerExportPlanRequest = {
        projectId,
        draft,
        ...(fields ? { acceptSuggestions: { fields } } : {}),
      };
      const result = await api.exportPlan(request);
      if (mine !== seq) return;
      batch(() => {
        planRequest = request;
        plan.value = result;
        planError.value = undefined;
        stale.value = false;
        const keep = selectedPath.peek();
        if (!keep || !result.files.some((f) => f.path === keep)) {
          selectedPath.value = result.files[0]?.path;
        }
      });
    } catch (thrown) {
      if (mine !== seq) return;
      batch(() => {
        planError.value = normalizeError(thrown);
        stale.value = false;
      });
    } finally {
      if (mine === seq) planning.value = false;
    }
  }

  function schedule(refetch: boolean): void {
    stale.value = true;
    cancelTimer?.();
    cancelTimer = sched.after(PLAN_IDLE_MS, () => {
      cancelTimer = undefined;
      void run(refetch);
    });
  }

  function clear(): void {
    cancelTimer?.();
    cancelTimer = undefined;
    seq += 1;
    batch(() => {
      plan.value = undefined;
      planError.value = undefined;
      planning.value = false;
      stale.value = false;
      suggestion.value = undefined;
      suggestError.value = undefined;
      acceptMode.value = 'none';
      if (customFields.peek().length > 0) customFields.value = [];
      selectedPath.value = undefined;
      flow.reset();
    });
    planRequest = undefined;
  }

  /**
   * Follow the open draft: a new draft resets everything, an edit asks for a new plan after a short
   * idle, and a change of the accepted values asks for the plan only. Returns the stop function.
   */
  function start(): () => void {
    let lastId: string | undefined;
    let lastDraft: DraftDto | undefined;
    let first = true;
    const stop = effect(() => {
      const id = editor.entryId.value;
      const draft = editor.draft.value;
      // read here so a change of the accepted values re-runs the effect
      void acceptMode.value;
      void customFields.value;
      if (id !== lastId || first) {
        first = false;
        lastId = id;
        lastDraft = draft;
        clear();
        if (draft) schedule(true);
        return;
      }
      const edited = draft !== lastDraft;
      lastDraft = draft;
      if (draft) schedule(edited);
      else clear();
    });
    return () => {
      stop();
      cancelTimer?.();
      cancelTimer = undefined;
    };
  }

  /** Switch the Combat Extended patch of the open draft on or off. */
  function setCeEnabled(on: boolean): void {
    const id = editor.entryId.peek() ?? '';
    editor.update((d) => {
      if (on) {
        return { ...d, spec: { ...d.spec, ce: stashes.get(id) ?? emptyCeBlock() } };
      }
      if (d.spec.ce) stashes.set(id, d.spec.ce);
      return { ...d, spec: setAt(d.spec, '/ce', undefined) };
    });
  }

  /** Write an answer into the Combat Extended block of the draft; undefined removes it. */
  function answer(pointer: Pointer, value: unknown): void {
    editor.update((d) => ({ ...d, spec: setCeValue(d.spec, pointer, value) }));
  }

  /** Choose which derived values the plan takes. */
  function setAcceptMode(mode: AcceptMode): void {
    acceptMode.value = mode;
  }

  /** Take or leave one derived value; the choice becomes a custom one. */
  function setAccepted(pointer: Pointer, on: boolean): void {
    const next = toggled(accepted.peek(), pointer, on);
    batch(() => {
      customFields.value = next;
      acceptMode.value = 'custom';
    });
  }

  function select(path: string): void {
    selectedPath.value = path;
  }

  return {
    plan,
    planError,
    planning,
    stale,
    suggestion,
    suggestError,
    acceptMode,
    accepted,
    selectedPath,
    apply: flow.apply,
    lastReport: flow.lastReport,
    ceOn,
    pending,
    hasChanges,
    blockedBy,
    canApply,
    start,
    setCeEnabled,
    answer,
    setAcceptMode,
    setAccepted,
    select,
    openApply: flow.open,
    closeApply: flow.close,
    showReport: flow.showReport,
    confirmApply: flow.confirm,
    /** Ask for a fresh plan now (after a failed call, for instance). */
    refresh: () => schedule(true),
  };
}

export type OutputStore = ReturnType<typeof createOutputStore>;
