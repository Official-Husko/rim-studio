import { signal } from '@preact/signals';
import type { ApiError, DiagnosticDto } from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import type { ProjectRef } from '~/shared/project';
import { planConversion, scanProject } from './api';
import { buildRequest } from './model';
import { withOpenProject } from './session';

/** The lint result of one weapon that carries a conversion. */
export interface LintRow {
  defName: string;
  file: string | undefined;
  diagnostics: DiagnosticDto[];
  error?: ApiError;
}

export type LintState =
  | { phase: 'idle' }
  | { phase: 'running'; done: number; total: number }
  | { phase: 'ready'; rows: LintRow[]; scanDiagnostics: DiagnosticDto[] }
  | { phase: 'error'; error: ApiError };

export const lint = signal<LintState>({ phase: 'idle' });

/** Counts up when the result is forgotten, so a view that shows it runs the lint again. */
export const lintEpoch = signal(0);

let sequence = 0;

/**
 * Lint the project's Combat Extended conversions: scan the project and plan every weapon that already
 * carries a conversion; the lint rules run inside that plan. Reads the project, writes nothing.
 */
export async function runLint(project: ProjectRef): Promise<void> {
  const mine = ++sequence;
  lint.value = { phase: 'running', done: 0, total: 0 };
  try {
    const found = await withOpenProject(project, (id) => scanProject(id, true));
    const converted = found.candidates.filter((c) => c.status === 'already-ce');
    const rows: LintRow[] = [];
    for (const [index, candidate] of converted.entries()) {
      if (mine !== sequence) return;
      lint.value = { phase: 'running', done: index, total: converted.length };
      try {
        const plan = await withOpenProject(project, (id) =>
          planConversion(
            id,
            candidate.kind ?? 'ranged',
            buildRequest(candidate, undefined, undefined),
          ),
        );
        rows.push({
          defName: candidate.defName,
          file: candidate.file,
          diagnostics: plan.diagnostics,
        });
      } catch (thrown) {
        rows.push({
          defName: candidate.defName,
          file: candidate.file,
          diagnostics: [],
          error: normalizeError(thrown),
        });
      }
    }
    if (mine !== sequence) return;
    lint.value = { phase: 'ready', rows, scanDiagnostics: found.diagnostics };
  } catch (thrown) {
    if (mine === sequence) lint.value = { phase: 'error', error: normalizeError(thrown) };
  }
}

/** Forget the lint result (the project changed). */
export function resetLint(): void {
  sequence += 1;
  lint.value = { phase: 'idle' };
  lintEpoch.value += 1;
}
