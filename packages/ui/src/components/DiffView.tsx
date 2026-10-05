import { useMemo } from 'preact/hooks';
import { cx } from '../cx';

export type DiffLineKind = 'meta' | 'hunk' | 'add' | 'remove' | 'context';

export interface DiffLine {
  kind: DiffLineKind;
  text: string;
  oldNo?: number;
  newNo?: number;
}

const HUNK = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/;

/** Parse unified diff text into display lines with line numbers (presentation only). */
export function parseUnifiedDiff(diff: string): DiffLine[] {
  const out: DiffLine[] = [];
  let oldNo = 0;
  let newNo = 0;
  let oldLeft = 0;
  let newLeft = 0;
  for (const raw of diff.split('\n')) {
    const hunk = HUNK.exec(raw);
    if (hunk) {
      oldNo = Number(hunk[1]);
      oldLeft = hunk[2] === undefined ? 1 : Number(hunk[2]);
      newNo = Number(hunk[3]);
      newLeft = hunk[4] === undefined ? 1 : Number(hunk[4]);
      out.push({ kind: 'hunk', text: raw });
    } else if (oldLeft <= 0 && newLeft <= 0) {
      // outside a hunk: file headers and noise
      if (raw !== '') out.push({ kind: 'meta', text: raw });
    } else if (raw.startsWith('+')) {
      out.push({ kind: 'add', text: raw.slice(1), newNo });
      newNo += 1;
      newLeft -= 1;
    } else if (raw.startsWith('-')) {
      out.push({ kind: 'remove', text: raw.slice(1), oldNo });
      oldNo += 1;
      oldLeft -= 1;
    } else if (raw.startsWith('\\')) {
      out.push({ kind: 'meta', text: raw });
    } else {
      out.push({ kind: 'context', text: raw.startsWith(' ') ? raw.slice(1) : raw, oldNo, newNo });
      oldNo += 1;
      newNo += 1;
      oldLeft -= 1;
      newLeft -= 1;
    }
  }
  return out;
}

export interface DiffViewProps {
  /** Unified diff text as produced by the backend. */
  diff: string;
  /** Accessible name of the diff region. */
  label: string;
  emptyText?: string;
}

const ROW: Record<DiffLineKind, string> = {
  add: 'bg-diff-added',
  remove: 'bg-diff-removed',
  context: '',
  hunk: 'bg-raised text-info',
  meta: 'text-faint',
};
const SIGN: Record<DiffLineKind, string> = {
  add: '+',
  remove: '-',
  context: ' ',
  hunk: '',
  meta: '',
};

/** A unified diff: added and removed lines are told apart by the sign as well as the tint. */
export function DiffView({ diff, label, emptyText = 'No changes' }: DiffViewProps) {
  const lines = useMemo(() => parseUnifiedDiff(diff), [diff]);
  if (lines.length === 0) {
    return (
      <div role="region" aria-label={label} class="border border-line bg-surface p-3 text-muted">
        {emptyText}
      </div>
    );
  }
  return (
    <div
      role="region"
      aria-label={label}
      class="overflow-auto border border-line bg-surface py-1 font-mono text-mono leading-5"
    >
      <div class="min-w-max">
        {lines.map((line, index) => (
          <div key={index} data-kind={line.kind} class={cx('flex px-2', ROW[line.kind])}>
            <span aria-hidden="true" class="w-10 shrink-0 pr-2 text-right text-faint select-none">
              {line.oldNo ?? ''}
            </span>
            <span aria-hidden="true" class="w-10 shrink-0 pr-2 text-right text-faint select-none">
              {line.newNo ?? ''}
            </span>
            <span
              aria-hidden={SIGN[line.kind] ? undefined : 'true'}
              class="w-4 shrink-0 select-none"
            >
              {SIGN[line.kind]}
            </span>
            <span class="whitespace-pre">{line.text || ' '}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
