import type {
  DiagnosticDto,
  DraftDto,
  ItemKindDto,
  SourcedDto,
  SuggestionDto,
  ValueSourceDto,
} from 'rimstudio-ipc-types';
import { ancestors, type Pointer } from './pointer';

/** The schema version the backend stores; a new draft starts at it. */
const DRAFT_SCHEMA_VERSION = 2;

/** An empty draft of a kind: identity only, calibration mode simple. The backend suggests the rest. */
export function newDraft(kind: ItemKindDto, defName: string, label: string): DraftDto {
  return {
    schemaVersion: DRAFT_SCHEMA_VERSION,
    kind,
    calibration: 'simple',
    spec: {
      kind,
      identity: { defName, label, description: '', modPrefix: '' },
      previewQuality: 'normal',
    },
  };
}

/** A number the user typed. */
export function typed(value: number): SourcedDto<number> {
  return { value, source: 'typed' };
}

/** A number taken from a suggestion; the backend still treats it as replaceable. */
export function suggested(value: number): SourcedDto<number> {
  return { value, source: 'suggested' };
}

/** The chip kind of a value source, or neutral for an answer of the dialogue. */
export function chipKind(source: ValueSourceDto): 'typed' | 'suggested' | 'anchor' | 'neutral' {
  return source === 'answered' ? 'neutral' : source;
}

/** Split a comma separated text into trimmed, non empty entries. */
export function splitList(text: string): string[] {
  return text
    .split(',')
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

/** Join list entries for a text field. */
export function joinList(list: readonly string[] | undefined): string {
  return (list ?? []).join(', ');
}

/** Diagnostics grouped by the field pointer they carry; those without one are under the empty key. */
export function groupByField(diagnostics: readonly DiagnosticDto[]): Map<Pointer, DiagnosticDto[]> {
  const out = new Map<Pointer, DiagnosticDto[]>();
  for (const d of diagnostics) {
    const key = d.field ?? '';
    const list = out.get(key);
    if (list) list.push(d);
    else out.set(key, [d]);
  }
  return out;
}

/** Diagnostics of a field and of the fields below it. */
export function diagnosticsFor(
  grouped: ReadonlyMap<Pointer, DiagnosticDto[]>,
  pointer: Pointer,
): DiagnosticDto[] {
  const out: DiagnosticDto[] = [];
  for (const [key, list] of grouped) {
    if (key === pointer || key.startsWith(`${pointer}/`)) out.push(...list);
  }
  return out;
}

/** Suggestions keyed by the field pointer they are for. */
export function suggestionsByField(
  suggestions: readonly SuggestionDto[] | undefined,
): Map<Pointer, SuggestionDto> {
  return new Map((suggestions ?? []).map((s) => [s.field, s]));
}

/**
 * The element of the form that a diagnostic pointer names. The pointer of a missing group such
 * as /tools has no field of its own, so the nearest field above or below it is used.
 */
export function findFieldElement(root: ParentNode, pointer: Pointer): HTMLElement | null {
  const all = Array.from(root.querySelectorAll<HTMLElement>('[data-field]'));
  for (const candidate of ancestors(pointer)) {
    const exact = all.find((el) => el.dataset.field === candidate);
    if (exact) return exact;
  }
  return all.find((el) => (el.dataset.field ?? '').startsWith(`${pointer}/`)) ?? null;
}

/** Move the keyboard focus to the field a pointer names. Returns false when there is none. */
export function focusField(root: ParentNode, pointer: Pointer): boolean {
  const holder = findFieldElement(root, pointer);
  if (!holder) return false;
  const control = holder.matches('input, textarea, select, button')
    ? holder
    : holder.querySelector<HTMLElement>('input, textarea, select, button');
  (control ?? holder).focus();
  holder.scrollIntoView?.({ block: 'center' });
  return true;
}
