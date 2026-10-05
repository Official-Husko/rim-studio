import { signal } from '@preact/signals';
import type { ApiError, ConvertCandidateDto, ConvertScanDto } from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import type { ProjectRef } from '~/shared/project';
import { scanProject } from './api';
import { isConvertible } from './model';
import { withOpenProject } from './session';

/** The state of the convert scan of the current project. */
export type ScanState =
  | { phase: 'idle' }
  | { phase: 'loading'; path: string; data?: ConvertScanDto }
  | { phase: 'ready'; path: string; data: ConvertScanDto }
  | { phase: 'error'; path: string; error: ApiError };

export const scan = signal<ScanState>({ phase: 'idle' });

/** The weapon shown in the detail pane. */
export const focused = signal<string | undefined>(undefined);

/** The weapons checked for conversion. */
export const checked = signal<ReadonlySet<string>>(new Set());

/** Whether weapons that already carry a conversion are listed. */
export const includeConverted = signal(true);

let sequence = 0;

/** Scan the project; a later call wins over an earlier one that is still running. */
export async function runScan(project: ProjectRef): Promise<void> {
  const mine = ++sequence;
  const before = scan.peek();
  const kept =
    (before.phase === 'ready' || before.phase === 'loading') && before.path === project.path
      ? before.data
      : undefined;
  scan.value = { phase: 'loading', path: project.path, ...(kept ? { data: kept } : {}) };
  try {
    const data = await withOpenProject(project, (id) => scanProject(id, includeConverted.peek()));
    if (mine !== sequence) return;
    scan.value = { phase: 'ready', path: project.path, data };
    const names = new Set(data.candidates.map((c) => c.defName));
    checked.value = new Set([...checked.peek()].filter((n) => names.has(n)));
    if (!focused.peek() || !names.has(focused.peek() ?? '')) {
      focused.value = data.candidates.find(isConvertible)?.defName ?? data.candidates[0]?.defName;
    }
  } catch (thrown) {
    if (mine !== sequence) return;
    scan.value = { phase: 'error', path: project.path, error: normalizeError(thrown) };
  }
}

/** The candidates of the last scan, or none. */
export function candidates(): readonly ConvertCandidateDto[] {
  const state = scan.peek();
  return (state.phase === 'ready' || state.phase === 'loading') && state.data
    ? state.data.candidates
    : [];
}

/** The focused candidate. */
export function focusedCandidate(): ConvertCandidateDto | undefined {
  const name = focused.peek();
  return candidates().find((c) => c.defName === name);
}

/** Check or uncheck one weapon. */
export function setChecked(defName: string, on: boolean): void {
  const next = new Set(checked.peek());
  if (on) next.add(defName);
  else next.delete(defName);
  checked.value = next;
}

/** Check every weapon that can be converted, or none. */
export function setAllChecked(on: boolean): void {
  checked.value = on
    ? new Set(
        candidates()
          .filter(isConvertible)
          .map((c) => c.defName),
      )
    : new Set();
}

/** Forget the scan (the project changed). */
export function resetScan(): void {
  sequence += 1;
  scan.value = { phase: 'idle' };
  focused.value = undefined;
  checked.value = new Set();
}
