import { batch, signal } from '@preact/signals';
import type {
  ApiError,
  ArchetypeCatalogDto,
  ArchetypeProposalDto,
  CeCalibreDto,
  DesignerArchetypeApplyResponse,
  DesignerArchetypeProposeRequest,
  DraftDto,
  DraftEntryDto,
} from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { newDraft } from '../../model/draft';
import { browserScheduler, type Scheduler } from '../../scheduler';
import {
  defaultDescriptors,
  emptyChoice,
  type DescriptorPatch,
  findArchetype,
  mergeDescriptors,
  modeOf,
  WIZARD_STEPS,
  type WizardChoice,
  type WizardStep,
} from './wizard-model';
import { createNameState } from './wizard-name';
import * as wizardApi from './wizard-api';
import { createEntry } from './wizard-finish';
import { buildRequest } from './wizard-request';

/** Milliseconds of silence before a changed descriptor is proposed again. */
export const PROPOSE_IDLE_MS = 120;

export interface WizardDeps {
  projectId: () => string | undefined;
  /** Called with the stored entry after a new draft is created. */
  onCreated: (entry: DraftEntryDto) => void;
  /** Called with the draft after a proposal was applied to an open draft (retune). */
  onApplied?: (draft: DraftDto) => void;
  scheduler?: Scheduler;
}

/**
 * The state of the weapon wizard: the catalogue, the choice, the live proposal and the name. The
 * proposal is read only and runs on every change; the draft is written once, on create or apply.
 */
export function createWizardStore(deps: WizardDeps) {
  const sched = deps.scheduler ?? browserScheduler;
  const catalog = signal<ArchetypeCatalogDto | undefined>(undefined);
  const catalogError = signal<ApiError | undefined>(undefined);
  const calibres = signal<CeCalibreDto[] | undefined>(undefined);
  const step = signal<WizardStep>('category');
  const choice = signal<WizardChoice>(emptyChoice());
  const proposal = signal<ArchetypeProposalDto | undefined>(undefined);
  /** The proposal before the last change, to show how the numbers moved. */
  const previous = signal<ArchetypeProposalDto | undefined>(undefined);
  const proposing = signal(false);
  const proposeError = signal<ApiError | undefined>(undefined);
  const name = createNameState();
  const { label, prefix, defName } = name;
  const busy = signal(false);
  const error = signal<ApiError | undefined>(undefined);
  /** Set when the wizard retunes an open draft instead of making a new one. */
  const target = signal<DraftDto | undefined>(undefined);
  /** The result of the last apply: which fields were filled and which kept their typed value. */
  const applied = signal<DesignerArchetypeApplyResponse | undefined>(undefined);

  let seq = 0;
  let cancelTimer: (() => void) | undefined;

  const archetype = () => findArchetype(catalog.peek(), choice.peek().archetypeId);

  async function loadCatalog(): Promise<void> {
    if (catalog.peek()) return;
    try {
      catalog.value = await wizardApi.archetypeCatalog(false);
      catalogError.value = undefined;
    } catch (thrown) {
      catalogError.value = normalizeError(thrown);
    }
  }

  /** The ammo sets of the Combat Extended install, loaded the first time they are wanted. */
  async function loadCalibres(): Promise<void> {
    if (calibres.peek()) return;
    try {
      const full = await wizardApi.archetypeCatalog(true);
      calibres.value = full.ce.calibres;
    } catch (thrown) {
      catalogError.value = normalizeError(thrown);
    }
  }

  function request(): DesignerArchetypeProposeRequest | undefined {
    return buildRequest(archetype(), choice.peek(), target.peek());
  }

  async function runPropose(): Promise<void> {
    cancelTimer?.();
    cancelTimer = undefined;
    const req = request();
    if (!req) return;
    const mine = ++seq;
    proposing.value = true;
    try {
      const result = await wizardApi.archetypePropose(req);
      if (mine !== seq) return;
      batch(() => {
        previous.value = proposal.peek();
        proposal.value = result;
        proposeError.value = undefined;
      });
    } catch (thrown) {
      if (mine === seq) proposeError.value = normalizeError(thrown);
    } finally {
      if (mine === seq) proposing.value = false;
    }
  }

  function schedulePropose(): void {
    cancelTimer?.();
    proposing.value = true;
    cancelTimer = sched.after(PROPOSE_IDLE_MS, () => void runPropose());
  }

  /** Pick a weapon type: its defaults become the descriptors and a first proposal is asked. */
  function chooseArchetype(id: string): void {
    const a = findArchetype(catalog.peek(), id);
    if (!a) return;
    batch(() => {
      choice.value = { ...choice.peek(), archetypeId: id, descriptors: defaultDescriptors(a) };
      proposal.value = undefined;
      previous.value = undefined;
    });
    schedulePropose();
  }

  /** Change descriptors; the proposal follows. */
  function setDescriptors(patch: DescriptorPatch): void {
    const c = choice.peek();
    choice.value = { ...c, descriptors: mergeDescriptors(c.descriptors, patch) };
    schedulePropose();
  }

  function setBalance(balance: WizardChoice['balance']): void {
    choice.value = { ...choice.peek(), balance };
    schedulePropose();
  }

  /** Switch between a calibre class and a Combat Extended ammo set. */
  function setCeCalibre(on: boolean): void {
    choice.value = { ...choice.peek(), ceCalibre: on };
    if (on) void loadCalibres();
    schedulePropose();
  }

  function go(next: WizardStep): void {
    step.value = next;
    if (next !== 'category' && proposal.peek() === undefined && !proposing.peek())
      void runPropose();
  }

  function move(delta: 1 | -1): void {
    const at = WIZARD_STEPS.indexOf(step.peek());
    const next = WIZARD_STEPS[at + delta];
    if (next) go(next);
  }

  /** Start from the numbers a draft already carries: the wizard retunes it. */
  function retune(draft: DraftDto): void {
    const saved = draft.archetype;
    if (!saved) return;
    batch(() => {
      target.value = draft;
      choice.value = {
        archetypeId: saved.archetype,
        descriptors: saved.descriptors,
        balance: saved.balance,
        ceCalibre: saved.mode === 'combat-extended',
      };
      step.value = 'describe';
    });
    if (saved.mode === 'combat-extended') void loadCalibres();
    void runPropose();
  }

  /** Wait for the proposal of the latest choice. */
  async function settle(): Promise<ArchetypeProposalDto | undefined> {
    if (cancelTimer || proposal.peek() === undefined) await runPropose();
    return proposal.peek();
  }

  /** Create the draft with the proposal applied, store it and hand it to the editor. */
  async function create(): Promise<boolean> {
    const a = archetype();
    const projectId = deps.projectId();
    if (!a || !projectId || defName.peek().trim() === '') return false;
    busy.value = true;
    try {
      const current = await settle();
      if (!current) return false;
      const blank = newDraft(a.kind, defName.peek().trim(), label.peek().trim());
      const created = await createEntry(projectId, blank, current);
      error.value = undefined;
      applied.value = created.response;
      deps.onCreated(created.entry);
      return true;
    } catch (thrown) {
      error.value = normalizeError(thrown);
      return false;
    } finally {
      busy.value = false;
    }
  }

  /** Apply the proposal to the open draft (retune): typed values stay. */
  async function applyToTarget(): Promise<boolean> {
    const open = target.peek();
    if (!open) return false;
    busy.value = true;
    try {
      const current = await settle();
      if (!current) return false;
      // the optional block is refreshed only when the draft already has it: the patch stays as it is
      const refreshCe = open.spec.ce !== undefined && modeOf(choice.peek()) === 'combat-extended';
      const response = await wizardApi.archetypeApply(open, current, refreshCe, true);
      error.value = undefined;
      applied.value = response;
      deps.onApplied?.(response.draft);
      return true;
    } catch (thrown) {
      error.value = normalizeError(thrown);
      return false;
    } finally {
      busy.value = false;
    }
  }

  /** Forget everything (the dialog closed). */
  function reset(): void {
    cancelTimer?.();
    cancelTimer = undefined;
    seq += 1;
    batch(() => {
      step.value = 'category';
      choice.value = emptyChoice();
      proposal.value = undefined;
      previous.value = undefined;
      proposing.value = false;
      proposeError.value = undefined;
      name.reset();
      busy.value = false;
      error.value = undefined;
      target.value = undefined;
      applied.value = undefined;
    });
  }

  return {
    catalog,
    catalogError,
    calibres,
    step,
    choice,
    proposal,
    previous,
    proposing,
    proposeError,
    label,
    prefix,
    defName,
    busy,
    error,
    target,
    applied,
    loadCatalog,
    loadCalibres,
    chooseArchetype,
    setDescriptors,
    setBalance,
    setCeCalibre,
    setLabel: name.setLabel,
    setPrefix: name.setPrefix,
    setDefName: name.setDefName,
    go,
    move,
    retune,
    create,
    applyToTarget,
    reset,
    archetype,
  };
}

export type WizardStore = ReturnType<typeof createWizardStore>;
