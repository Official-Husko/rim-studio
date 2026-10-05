import type { DiagnosticDto } from 'rimstudio-ipc-types';
import { Badge, type BadgeTone } from 'rimstudio-ui';
import { severityText } from './DiagnosticNotes';

const TONE: Record<DiagnosticDto['severity'], BadgeTone> = {
  error: 'danger',
  warning: 'warning',
  info: 'info',
  hint: 'neutral',
};

/** The same problem told twice (by the file facts and by the plan) is shown once. */
export function mergeProblems(...lists: ReadonlyArray<readonly DiagnosticDto[]>): DiagnosticDto[] {
  const seen = new Set<string>();
  const out: DiagnosticDto[] = [];
  for (const d of lists.flat()) {
    const key = `${d.code}|${d.message}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(d);
  }
  return out;
}

export interface AssetProblemsProps {
  diagnostics: readonly DiagnosticDto[];
}

/** Every problem of an import with its severity word and code, errors included. */
export function AssetProblems({ diagnostics }: AssetProblemsProps) {
  if (diagnostics.length === 0) return null;
  return (
    <ul class="flex flex-col gap-1">
      {diagnostics.map((d, i) => (
        <li key={`${d.code}-${i}`} class="flex items-start gap-2 text-small text-muted">
          <Badge tone={TONE[d.severity]}>{severityText(d.severity)}</Badge>
          <span class="min-w-0 break-words">
            {d.message} <span class="font-mono text-mono-small text-faint">{d.code}</span>
          </span>
        </li>
      ))}
    </ul>
  );
}
