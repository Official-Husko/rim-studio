import { useEffect, useRef, useState } from 'preact/hooks';
import { Button, IconButton, NumberField, TextField } from 'rimstudio-ui';
import { diagnosticsFor } from '../../model/draft';
import { getAt, type Pointer } from '../../model/pointer';
import { DiagnosticNotes } from './DiagnosticNotes';
import { useFieldEnv } from './fieldEnv';

export interface NumberMapFieldProps {
  /** Where the map of names to numbers lives in the spec. */
  pointer: Pointer;
  /** Heading of the list. */
  label: string;
  help?: string;
  /** Accessible name of the name column, for example Skill. */
  keyLabel: string;
  /** Accessible name of the number column, for example Level. */
  valueLabel: string;
  addLabel: string;
  /** Accessible name of a remove button for a row. */
  removeLabel: (key: string) => string;
  step?: number;
}

interface Row {
  id: number;
  key: string;
  value: number | undefined;
}

function toMap(rows: readonly Row[]): Record<string, number> {
  const map: Record<string, number> = {};
  for (const { key, value } of rows) {
    const name = key.trim();
    if (name !== '' && value !== undefined && !(name in map)) map[name] = value;
  }
  return map;
}

function toRows(map: Record<string, number> | undefined, next: () => number): Row[] {
  return Object.entries(map ?? {}).map(([key, value]) => ({ id: next(), key, value }));
}

/**
 * A map of names to numbers edited as rows (skill and level, stat and offset). A row without a
 * name or a number stays on screen but is not written until it is complete.
 */
export function NumberMapField({
  pointer,
  label,
  help,
  keyLabel,
  valueLabel,
  addLabel,
  removeLabel,
  step = 1,
}: NumberMapFieldProps) {
  const env = useFieldEnv();
  const stored = getAt(env.spec, pointer) as Record<string, number> | undefined;
  const counter = useRef(0);
  const nextId = (): number => (counter.current += 1);
  const [rows, setRows] = useState<Row[]>(() => toRows(stored, nextId));
  const storedText = JSON.stringify(stored ?? {});

  useEffect(() => {
    // an outside change (a clone opened, a quiz answer) replaces the rows; our own writes do not
    if (JSON.stringify(toMap(rows)) !== storedText) setRows(toRows(stored, nextId));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [storedText]);

  const diagnostics = diagnosticsFor(env.diagnostics, pointer);
  const commit = (next: Row[]): void => {
    setRows(next);
    const map = toMap(next);
    env.setField(pointer, Object.keys(map).length === 0 ? undefined : map);
  };
  const change = (id: number, patch: Partial<Row>): void =>
    commit(rows.map((row) => (row.id === id ? { ...row, ...patch } : row)));

  return (
    <div data-field={pointer} class="flex min-w-0 flex-col gap-2">
      <h3 class="font-display text-label tracking-label text-muted uppercase">{label}</h3>
      {help ? <p class="text-small text-faint">{help}</p> : null}
      {rows.map((row, index) => (
        <div key={row.id} class="flex items-center gap-2">
          <div class="min-w-0 flex-1">
            <TextField
              aria-label={`${keyLabel} ${index + 1}`}
              value={row.key}
              onValueChange={(key) => change(row.id, { key })}
            />
          </div>
          <div class="w-28 shrink-0">
            <NumberField
              aria-label={`${valueLabel} ${index + 1}`}
              step={step}
              value={row.value}
              onValueChange={(value) => change(row.id, { value })}
            />
          </div>
          <IconButton
            icon="trash"
            variant="danger"
            label={removeLabel(row.key || String(index + 1))}
            onClick={() => commit(rows.filter((r) => r.id !== row.id))}
          />
        </div>
      ))}
      <div>
        <Button
          size="sm"
          icon="plus"
          onClick={() => setRows([...rows, { id: nextId(), key: '', value: undefined }])}
        >
          {addLabel}
        </Button>
      </div>
      <DiagnosticNotes diagnostics={diagnostics} />
    </div>
  );
}
