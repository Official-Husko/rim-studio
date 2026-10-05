import type {
  CePatchSpecDto,
  CeSuggestionDto,
  DesignSpecDto,
  DiagnosticDto,
  SourcedDto,
  ValueSourceDto,
  WritePlanDto,
} from 'rimstudio-ipc-types';
import { setAt, type Pointer } from './model/pointer';

// Presentation helpers of the output panel: they sort what the backend returned and map a form
// answer onto the draft. The numbers, the ratings and the rules all come from the backend.

/** Which derived Combat Extended values the plan takes. */
export type AcceptMode = 'none' | 'reliable' | 'all' | 'custom';

/** The Combat Extended block of a freshly switched on draft: the two flags are always present. */
export function emptyCeBlock(): CePatchSpecDto {
  return { oneHanded: false, beltFed: false };
}

/** True when the draft carries the Combat Extended block, which is the switch. */
export function ceIsOn(spec: DesignSpecDto): boolean {
  return spec.ce !== undefined;
}

/** A value with the source that says who gave it. */
export function sourced(value: number, source: ValueSourceDto): SourcedDto<number> {
  return { value, source };
}

const TOOL_POINTER = /^\/ce\/toolPenetration\/([^/]+)\/(sharp|blunt)$/;

function unescape(segment: string): string {
  return segment.replace(/~1/g, '/').replace(/~0/g, '~');
}

/**
 * Write one answer into the Combat Extended block of a spec. Tool penetration pointers name the
 * tool, not an index, so they are mapped to the entry of the list; every other pointer is a plain
 * path. A value of undefined removes the answer.
 */
export function setCeValue(spec: DesignSpecDto, pointer: Pointer, value: unknown): DesignSpecDto {
  const match = TOOL_POINTER.exec(pointer);
  if (!match) return setAt(spec, pointer, value);
  const tool = unescape(match[1] ?? '');
  const part = match[2] === 'sharp' ? 'sharp' : 'blunt';
  const list = [...(spec.ce?.toolPenetration ?? [])];
  const index = list.findIndex((entry) => entry.tool === tool);
  const current = index >= 0 ? list[index] : undefined;
  const next = { ...(current ?? { tool }) };
  if (value === undefined) delete next[part];
  else next[part] = value as SourcedDto<number>;
  const empty = next.sharp === undefined && next.blunt === undefined;
  if (index >= 0) {
    if (empty) list.splice(index, 1);
    else list[index] = next;
  } else if (!empty) {
    list.push(next);
  }
  return setAt(spec, '/ce/toolPenetration', list.length > 0 ? list : undefined);
}

/**
 * The pointers a plan can take from the suggestion: every derived field, or only those the backend
 * rated reliable. The default projectile follows the chosen ammo set and is taken in both cases.
 */
export function derivedPointers(
  suggestion: CeSuggestionDto | undefined,
  onlyReliable: boolean,
): Pointer[] {
  if (!suggestion) return [];
  const numbers = suggestion.fields
    .filter((f) => f.status === 'derived' && (!onlyReliable || f.rating === 'reliable'))
    .map((f) => f.field);
  const choices = suggestion.choices
    .filter((c) => c.status === 'derived' && c.value !== undefined)
    .map((c) => c.field);
  return [...numbers, ...choices];
}

/**
 * The accepted fields to send with a plan request. An empty list means every derived field to the
 * backend, so "none" is expressed by sending no option at all (undefined).
 */
export function acceptedFor(
  on: boolean,
  mode: AcceptMode,
  custom: readonly string[],
  suggestion: CeSuggestionDto | undefined,
): string[] | undefined {
  if (!on || mode === 'none') return undefined;
  if (mode === 'all') return [];
  const list = mode === 'reliable' ? derivedPointers(suggestion, true) : [...custom];
  return list.length > 0 ? list : undefined;
}

/** The diagnostics that belong to the lint of the generated patch. */
export function isLint(d: DiagnosticDto): boolean {
  return d.code.startsWith('ce.cep') || d.code === 'ce.not-checked';
}

/** The diagnostics that list a number the patch took from the suggestion. */
export function isDerived(d: DiagnosticDto): boolean {
  return d.code === 'ce.derived-value';
}

/**
 * A required Combat Extended field that has no value in the draft. The plan may still fill it from
 * a derived value the user takes, so the Output panel (not the editor) decides if it is a problem.
 */
export function isCeRequiredMissing(d: DiagnosticDto): boolean {
  return d.code === 'design.required-missing' && (d.field ?? '').startsWith('/ce/');
}

/** The answers the plan still waits for: needs-answer and missing required Combat Extended fields. */
export function pendingAnswers(plan: WritePlanDto | undefined): DiagnosticDto[] {
  const seen = new Set<string>();
  return (plan?.diagnostics ?? []).filter((d) => {
    const field = d.field ?? '';
    const waiting =
      d.severity === 'error' &&
      field.startsWith('/ce/') &&
      (d.code === 'designer.ce-needs-answer' || isCeRequiredMissing(d));
    if (!waiting || seen.has(field)) return false;
    seen.add(field);
    return true;
  });
}

/** The problems list: everything except the derived values and the lint, which have their own lists. */
export function problemsOf(plan: WritePlanDto | undefined): DiagnosticDto[] {
  const order = { error: 0, warning: 1, info: 2, hint: 3 } as const;
  return (plan?.diagnostics ?? [])
    .filter((d) => !isDerived(d) && !isLint(d))
    .sort((a, b) => order[a.severity] - order[b.severity]);
}

/** The folder of a path, or an empty string at the root. */
export function folderOf(path: string): string {
  const cut = path.lastIndexOf('/');
  return cut < 0 ? '' : path.slice(0, cut);
}

/** The file name of a path. */
export function nameOf(path: string): string {
  return path.slice(path.lastIndexOf('/') + 1);
}

/** The accepted pointer set that results from toggling one pointer in the current effective set. */
export function toggled(current: readonly string[], pointer: string, on: boolean): string[] {
  const rest = current.filter((p) => p !== pointer);
  return on ? [...rest, pointer] : rest;
}

/** The pointers the plan takes right now, as the checkboxes of the rows show them. */
export function effectiveAccepted(
  mode: AcceptMode,
  custom: readonly string[],
  suggestion: CeSuggestionDto | undefined,
): string[] {
  if (mode === 'all') return derivedPointers(suggestion, false);
  if (mode === 'reliable') return derivedPointers(suggestion, true);
  if (mode === 'custom') return [...custom];
  return [];
}
