import { batch, signal } from '@preact/signals';
import type {
  ApiError,
  DesignerCloneDiffResponse,
  DesignerStructureDefaultsResponse,
  DraftDto,
  DraftEntryDto,
  FitReportDto,
  PreviewDto,
} from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import * as api from './api';
import { suggested } from './model/draft';
import { getAt, setAt, type Pointer } from './model/pointer';
import { AUTOSAVE_IDLE_MS, browserScheduler, SLOW_IDLE_MS, type Scheduler } from './scheduler';

/** Where the open draft stands against the stored copy. */
export type SaveState = 'idle' | 'dirty' | 'saving' | 'saved' | 'error';

export interface EditorDeps {
  /** The id of the current project; undefined when none is open. */
  projectId: () => string | undefined;
  scheduler?: Scheduler;
  /** Called after every successful save with the stored entry. */
  onSaved?: (entry: DraftEntryDto) => void;
}

/**
 * The open draft with its live results. Edits run one preview per animation frame; the fit, the
 * clone diff and the structure defaults follow after a short idle; the draft is saved on blur or
 * after a longer idle. Results of an older call never replace newer ones.
 */
export function createEditorStore(deps: EditorDeps) {
  const sched = deps.scheduler ?? browserScheduler;
  const draft = signal<DraftDto | undefined>(undefined);
  const entryId = signal<string | undefined>(undefined);
  const saveState = signal<SaveState>('idle');
  const savedAtMs = signal<number | undefined>(undefined);
  const preview = signal<PreviewDto | undefined>(undefined);
  const fit = signal<FitReportDto | undefined>(undefined);
  const diff = signal<DesignerCloneDiffResponse | undefined>(undefined);
  const structure = signal<DesignerStructureDefaultsResponse | undefined>(undefined);
  const error = signal<ApiError | undefined>(undefined);
  const previewing = signal(false);
  const ownBusy = signal(false);
  const projectileNotes = signal<string[]>([]);

  let rev = 0;
  let seq = 0;
  let slowSeq = 0;
  let saving = false;
  let cancelFrame: (() => void) | undefined;
  let cancelSlow: (() => void) | undefined;
  let cancelSave: (() => void) | undefined;

  const fail = (thrown: unknown): void => {
    error.value = normalizeError(thrown);
  };

  async function runPreview(): Promise<void> {
    const current = draft.peek();
    if (!current) return;
    const mine = ++seq;
    previewing.value = true;
    try {
      const result = await api.preview(current);
      if (mine === seq) {
        preview.value = result;
        error.value = undefined;
      }
    } catch (thrown) {
      if (mine === seq) fail(thrown);
    } finally {
      if (mine === seq) previewing.value = false;
    }
  }

  async function runSlow(): Promise<void> {
    const current = draft.peek();
    if (!current) return;
    const mine = ++slowSeq;
    const [f, d, s] = await Promise.allSettled([
      api.fit(current),
      current.clonedFrom ? api.cloneDiff(current) : Promise.resolve(undefined),
      api.structureDefaults(current),
    ]);
    if (mine !== slowSeq) return;
    batch(() => {
      if (f.status === 'fulfilled') fit.value = f.value;
      if (d.status === 'fulfilled') diff.value = d.value;
      if (s.status === 'fulfilled') structure.value = s.value;
    });
  }

  function scheduleResults(): void {
    cancelFrame ??= sched.frame(() => {
      cancelFrame = undefined;
      void runPreview();
    });
    cancelSlow?.();
    cancelSlow = sched.after(SLOW_IDLE_MS, () => {
      cancelSlow = undefined;
      void runSlow();
    });
  }

  function armAutosave(): void {
    cancelSave?.();
    cancelSave = sched.after(AUTOSAVE_IDLE_MS, () => {
      cancelSave = undefined;
      void flush();
    });
  }

  function cancelAll(): void {
    cancelFrame?.();
    cancelSlow?.();
    cancelSave?.();
    cancelFrame = cancelSlow = cancelSave = undefined;
  }

  /** Save the draft now when it has unsaved changes. */
  async function flush(): Promise<void> {
    cancelSave?.();
    cancelSave = undefined;
    const current = draft.peek();
    const projectId = deps.projectId();
    if (!current || !projectId || saveState.peek() !== 'dirty' || saving) return;
    saving = true;
    const savedRev = rev;
    saveState.value = 'saving';
    try {
      const response = await api.draftSave(projectId, current, entryId.peek());
      entryId.value = response.id;
      savedAtMs.value = response.savedAtMs;
      deps.onSaved?.({
        id: response.id,
        defName: current.spec.identity.defName,
        label: current.spec.identity.label,
        kind: current.kind,
        updatedAtMs: response.savedAtMs,
        draft: current,
      });
      saveState.value = rev === savedRev ? 'saved' : 'dirty';
      if (rev !== savedRev) armAutosave();
    } catch (thrown) {
      fail(thrown);
      saveState.value = 'error';
    } finally {
      saving = false;
    }
  }

  /** Make a stored draft the open one. */
  function open(entry: DraftEntryDto): void {
    cancelAll();
    seq += 1;
    rev += 1;
    batch(() => {
      draft.value = entry.draft;
      entryId.value = entry.id;
      saveState.value = 'saved';
      savedAtMs.value = entry.updatedAtMs;
      preview.value = undefined;
      fit.value = undefined;
      diff.value = undefined;
      structure.value = undefined;
      error.value = undefined;
      projectileNotes.value = [];
    });
    scheduleResults();
  }

  /** Close the open draft (the caller flushes first when it cares). */
  function close(): void {
    cancelAll();
    seq += 1;
    batch(() => {
      draft.value = undefined;
      entryId.value = undefined;
      saveState.value = 'idle';
      preview.value = undefined;
      fit.value = undefined;
      diff.value = undefined;
      structure.value = undefined;
    });
  }

  /** Replace the open draft by an edited one. */
  function edit(next: DraftDto): void {
    if (!draft.peek()) return;
    rev += 1;
    batch(() => {
      draft.value = next;
      saveState.value = 'dirty';
    });
    scheduleResults();
    armAutosave();
  }

  /** Change the draft with a function of the current one. */
  function update(change: (current: DraftDto) => DraftDto): void {
    const current = draft.peek();
    if (current) edit(change(current));
  }

  /** Write a value into the spec at a pointer; undefined removes it. */
  function setField(pointer: Pointer, value: unknown): void {
    update((d) => ({ ...d, spec: setAt(d.spec, pointer, value) }));
  }

  /** Fill the empty fields (and fields that only hold a suggestion) from the preview suggestions. */
  function fillFromEstimate(): number {
    const current = draft.peek();
    const suggestions = preview.peek()?.suggestions;
    if (!current || !suggestions) return 0;
    let spec = current.spec;
    let filled = 0;
    for (const s of suggestions) {
      if (s.locked || s.value === undefined) continue;
      const have = getAt(spec, s.field) as { source?: string } | undefined;
      if (have !== undefined && have.source !== 'suggested') continue;
      const parent = s.field.replace(/\/[^/]+$/, '');
      if (parent !== '' && parent !== s.field && getAt(spec, parent) === undefined) continue;
      spec = setAt(spec, s.field, suggested(s.value));
      filled += 1;
    }
    if (filled > 0) edit({ ...current, spec });
    return filled;
  }

  /**
   * Give the open gun draft a projectile of its own, or point it back at the shared one. The
   * backend builds the projectile; when the draft changed meanwhile only its projectile is taken.
   */
  async function setOwnProjectile(own: boolean): Promise<void> {
    const before = draft.peek();
    if (!before || ownBusy.peek()) return;
    ownBusy.value = true;
    try {
      const response = await api.projectileOwn(before, own);
      const now = draft.peek();
      if (now === before) edit(response.draft);
      else if (now) setField('/ranged/projectile', response.draft.spec.ranged?.projectile);
      projectileNotes.value = response.notes ?? [];
      error.value = undefined;
    } catch (thrown) {
      fail(thrown);
    } finally {
      ownBusy.value = false;
    }
  }

  /** Take the structure the backend suggested (parent, projectile, cost list, stuff). */
  function applyStructure(): void {
    const suggestion = structure.peek();
    if (suggestion) edit(suggestion.draft);
  }

  return {
    draft,
    entryId,
    saveState,
    savedAtMs,
    preview,
    fit,
    diff,
    structure,
    error,
    previewing,
    ownBusy,
    projectileNotes,
    open,
    close,
    edit,
    update,
    setField,
    flush,
    fillFromEstimate,
    applyStructure,
    setOwnProjectile,
    /** Ask for fresh results now (after a calibration, for instance). */
    refresh: scheduleResults,
  };
}

export type EditorStore = ReturnType<typeof createEditorStore>;
