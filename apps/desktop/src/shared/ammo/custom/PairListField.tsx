import type { DiagnosticDto } from 'rimstudio-ipc-types';
import { Button, IconButton, NumberField, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

export interface PairRow {
  name: string;
  amount: number | undefined;
}

export interface PairListFieldProps {
  label: string;
  nameLabel: string;
  amountLabel: string;
  addLabel: string;
  help?: string;
  rows: readonly PairRow[];
  onChange: (rows: PairRow[]) => void;
  /** Diagnostics of the rows, shown below the list. */
  diagnostics?: readonly DiagnosticDto[];
  whole?: boolean;
  /** The pointer of the list, so a diagnostic can find it. */
  pointer: string;
}

/** A list of rows with a name and an amount, such as the secondary damage or the fragments of a projectile. */
export function PairListField({
  label,
  nameLabel,
  amountLabel,
  addLabel,
  help,
  rows,
  onChange,
  diagnostics = [],
  whole,
  pointer,
}: PairListFieldProps) {
  const replace = (index: number, row: PairRow | undefined): void => {
    const next = [...rows];
    if (row) next[index] = row;
    else next.splice(index, 1);
    onChange(next);
  };
  return (
    <div class="flex flex-col gap-2" role="group" aria-label={label} data-ammo-field={pointer}>
      <span class="text-small font-semibold text-muted">{label}</span>
      {help ? <p class="m-0 text-small text-faint">{help}</p> : null}
      {rows.map((row, index) => (
        <div key={index} class="flex flex-wrap items-center gap-2">
          <div class="min-w-40 flex-1">
            <TextField
              aria-label={`${nameLabel} ${index + 1}`}
              value={row.name}
              onValueChange={(name) => replace(index, { ...row, name })}
            />
          </div>
          <div class="w-32">
            <NumberField
              aria-label={`${amountLabel} ${index + 1}`}
              value={row.amount}
              onValueChange={(amount) => replace(index, { ...row, amount })}
              step={whole ? 1 : 0.1}
            />
          </div>
          <IconButton
            icon="trash"
            label={t('ammo.pairs.remove', { n: index + 1 })}
            onClick={() => replace(index, undefined)}
          />
        </div>
      ))}
      <div class="self-start">
        <Button
          size="sm"
          icon="plus"
          onClick={() => onChange([...rows, { name: '', amount: undefined }])}
        >
          {addLabel}
        </Button>
      </div>
      {diagnostics.map((d, i) => (
        <p
          key={`${d.code}-${i}`}
          class={
            d.severity === 'error' ? 'm-0 text-small text-danger' : 'm-0 text-small text-warning'
          }
        >
          {d.message}
        </p>
      ))}
    </div>
  );
}
