import { batch, computed, signal } from '@preact/signals';
import type {
  ApiError,
  CeAmmoSuggestionDto,
  CustomAmmoDto,
  CustomAmmoTypeDto,
  WritePlanDto,
} from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { ammoSuggest } from '../api';
import { createSuggester } from './suggester';
import { getAt } from '../pointer';
import {
  cleaned,
  duplicateOf,
  emptyCustom,
  emptyType,
  isAmmoDiagnostic,
  newId,
  placeOf,
  type Section,
  type TypeTab,
} from './model';

export interface CustomAmmoDeps {
  /** The custom ammo being edited; a new caliber when absent. */
  initial?: CustomAmmoDto | undefined;
  /**
   * Plans the weapon with this custom ammo, so the backend can check it and list the files. Absent when no
   * project is open: nothing can be checked then.
   */
  plan?: ((custom: CustomAmmoDto) => Promise<WritePlanDto>) | undefined;
  suggest?: typeof ammoSuggest;
  /** Milliseconds of silence after an edit before the plan is asked again. */
  debounceMs?: number;
}

/**
 * The working copy of one custom caliber and everything around it: the section and the type being edited,
 * the suggestions of the backend and the plan that checks the spec. Nothing is saved from here; the host
 * takes `custom` when the user presses Save.
 */
export function createCustomAmmoStore(deps: CustomAmmoDeps = {}) {
  const suggest = deps.suggest ?? ammoSuggest;
  const debounceMs = deps.debounceMs ?? 350;
  const initial = deps.initial ? structuredClone(deps.initial) : emptyCustom();

  const custom = signal<CustomAmmoDto>(initial);
  const ids = signal<string[]>(initial.types.map(newId));
  const section = signal<Section>('identity');
  const typeIndex = signal(0);
  const tab = signal<TypeTab>('projectile');
  const plan = signal<WritePlanDto | undefined>(undefined);
  const planError = signal<ApiError | undefined>(undefined);
  const planning = signal(false);
  const stale = signal(false);
  const suggestions = signal<Readonly<Record<string, CeAmmoSuggestionDto>>>({});
  const suggesting = signal<string | undefined>(undefined);
  const suggestError = signal<ApiError | undefined>(undefined);
  /** The field the user asked to see (from a diagnostic); the view focuses it and clears it. */
  const focusField = signal<string | undefined>(undefined);
  const hasPlan = deps.plan !== undefined;

  const { suggestFor, suggestionFor, applySuggestion } = createSuggester({
    custom,
    ids,
    suggestions,
    suggesting,
    suggestError,
    suggest,
    updateType: (index, change) => updateType(index, change),
    setField: (index, pointer, value) => setField(index, pointer, value),
  });

  let seq = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const diagnostics = computed(() => (plan.value?.diagnostics ?? []).filter(isAmmoDiagnostic));
  const errors = computed(() => diagnostics.value.filter((d) => d.severity === 'error'));
  const ammoFile = computed(() => plan.value?.files.find((f) => f.kind === 'ce-defs'));
  /** Errors of the weapon plan that are not about the ammunition; they can hide the file list. */
  const otherErrors = computed(() =>
    (plan.value?.diagnostics ?? []).filter((d) => d.severity === 'error' && !isAmmoDiagnostic(d)),
  );
  const canSave = computed(
    () =>
      custom.value.types.length > 0 && errors.value.length === 0 && !planning.value && !stale.value,
  );

  async function run(): Promise<void> {
    if (!deps.plan) return;
    const mine = ++seq;
    planning.value = true;
    try {
      const result = await deps.plan(custom.peek());
      if (mine !== seq) return;
      batch(() => {
        plan.value = result;
        planError.value = undefined;
        stale.value = false;
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

  /** Ask for the plan after a pause; also used for the first check when the window opens. */
  function schedule(): void {
    if (!deps.plan) return;
    stale.value = true;
    if (timer !== undefined) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
      void run();
    }, debounceMs);
  }

  function set(next: CustomAmmoDto): void {
    custom.value = next;
    schedule();
  }

  /** Change the members of the caliber itself; an empty text or an undefined member is removed. */
  function patch(change: Partial<Omit<CustomAmmoDto, 'types'>>): void {
    const next: Record<string, unknown> = { ...custom.peek() };
    for (const [key, value] of Object.entries(change)) {
      if (value === undefined || value === '' || (Array.isArray(value) && value.length === 0)) {
        delete next[key];
      } else next[key] = value;
    }
    set(next as unknown as CustomAmmoDto);
  }

  function updateType(index: number, change: (type: CustomAmmoTypeDto) => CustomAmmoTypeDto): void {
    const types = custom.peek().types;
    const held = types[index];
    if (!held) return;
    const copy = [...types];
    copy[index] = change(held);
    set({ ...custom.peek(), types: copy });
  }

  /** Set (or clear) one field of one type by its pointer inside the type, such as `/projectile/damage`. */
  function setField(index: number, pointer: string, value: unknown): void {
    updateType(index, (type) => cleaned(type, pointer, value));
  }

  function selectType(index: number): void {
    typeIndex.value = Math.max(0, Math.min(index, custom.peek().types.length - 1));
  }

  /** Add a type; with a class the backend fills it from the nearest of the user's own ammunition. */
  function addType(ammoClass = ''): void {
    const types = [...custom.peek().types, emptyType(ammoClass)];
    batch(() => {
      ids.value = [...ids.peek(), newId()];
      set({ ...custom.peek(), types });
      typeIndex.value = types.length - 1;
      tab.value = 'projectile';
    });
    if (ammoClass !== '') void suggestFor(types.length - 1);
  }

  /** Change the ammo class of a type; a type with no values yet is filled from the suggestion. */
  function setClass(index: number, ammoClass: string): void {
    const type = custom.peek().types[index];
    if (!type) return;
    const bare = Object.keys(type.projectile).length === 0 && Object.keys(type.item).length === 0;
    updateType(index, (held) => ({ ...held, ammoClass }));
    if (bare && ammoClass !== '') void suggestFor(index);
  }

  function removeType(index: number): void {
    const types = custom.peek().types.filter((_, i) => i !== index);
    batch(() => {
      ids.value = ids.peek().filter((_, i) => i !== index);
      const whole = custom.peek();
      const gone = whole.types[index];
      const defaults =
        gone && whole.defaultType !== undefined && whole.defaultType === gone.key.trim()
          ? { defaultType: undefined }
          : {};
      set({ ...whole, ...defaults, types });
      typeIndex.value = Math.max(0, Math.min(typeIndex.peek(), types.length - 1));
    });
  }

  function duplicateType(index: number): void {
    const whole = custom.peek();
    const source = whole.types[index];
    if (!source) return;
    const copy = duplicateOf(
      source,
      whole.types.map((x) => x.key),
    );
    const types = [...whole.types];
    types.splice(index + 1, 0, copy);
    const list = [...ids.peek()];
    list.splice(index + 1, 0, newId());
    batch(() => {
      ids.value = list;
      set({ ...whole, types });
      typeIndex.value = index + 1;
    });
  }

  function moveType(index: number, delta: number): void {
    const to = index + delta;
    const whole = custom.peek();
    if (to < 0 || to >= whole.types.length) return;
    const types = [...whole.types];
    const [moved] = types.splice(index, 1);
    const list = [...ids.peek()];
    const [movedId] = list.splice(index, 1);
    if (!moved || movedId === undefined) return;
    types.splice(to, 0, moved);
    list.splice(to, 0, movedId);
    batch(() => {
      ids.value = list;
      set({ ...whole, types });
      typeIndex.value = to;
    });
  }

  /** What the type holds at a pointer, for the form. */
  function valueAt(index: number, pointer: string): unknown {
    return getAt(custom.value.types[index], pointer);
  }

  /** Show the field a diagnostic names: the section, the type and the tab, then ask the view to focus it. */
  function goTo(pointer: string | undefined): void {
    const place = placeOf(pointer);
    batch(() => {
      section.value = place.section;
      if (place.typeIndex !== undefined) selectType(place.typeIndex);
      if (place.tab) tab.value = place.tab;
      focusField.value = pointer;
    });
  }

  function dispose(): void {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
    seq += 1;
  }

  return {
    custom,
    ids,
    section,
    typeIndex,
    tab,
    plan,
    planError,
    planning,
    stale,
    suggestions,
    suggesting,
    suggestError,
    focusField,
    hasPlan,
    diagnostics,
    errors,
    ammoFile,
    otherErrors,
    canSave,
    patch,
    setField,
    setClass,
    selectType,
    suggestFor,
    addType,
    removeType,
    duplicateType,
    moveType,
    suggestionFor,
    applySuggestion,
    valueAt,
    goTo,
    check: () => {
      if (timer !== undefined) clearTimeout(timer);
      timer = undefined;
      stale.value = hasPlan;
      void run();
    },
    dispose,
  };
}

export type CustomAmmoStore = ReturnType<typeof createCustomAmmoStore>;
