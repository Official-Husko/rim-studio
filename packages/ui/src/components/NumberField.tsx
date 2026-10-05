import type { ComponentChildren } from 'preact';
import { useState } from 'preact/hooks';
import { cx } from '../cx';
import { useFieldContext } from './fieldContext';

export interface NumberFieldProps {
  /** The committed value; undefined shows an empty field. */
  value: number | undefined;
  onValueChange: (value: number | undefined) => void;
  /** Use FormField for a visible label, or pass an aria-label. */
  'aria-label'?: string;
  id?: string;
  /** Arrow key step; Shift multiplies by 10, PageUp and PageDown step by 10. */
  step?: number;
  min?: number;
  max?: number;
  /** Unit text shown inside the field after the number, such as "m" or "kg". */
  unit?: string;
  /** Slot for a source chip (typed, suggested, anchor, derived). */
  chip?: ComponentChildren;
  invalid?: boolean;
  disabled?: boolean;
  readOnly?: boolean;
  placeholder?: string;
}

/** Decimal places needed to represent a step exactly. */
function decimalsOf(step: number): number {
  const text = String(step);
  if (text.includes('e-')) return Number(text.split('e-')[1] ?? 0);
  return text.split('.')[1]?.length ?? 0;
}

function clamp(n: number, min?: number, max?: number): number {
  let out = n;
  if (min !== undefined && out < min) out = min;
  if (max !== undefined && out > max) out = max;
  return out;
}

function parse(text: string): number | undefined {
  const t = text.trim().replace(',', '.');
  if (t === '' || t === '-' || t === '.') return undefined;
  const n = Number(t);
  return Number.isFinite(n) ? n : undefined;
}

/** A numeric input with a unit, stepping on arrow keys and a slot for a source chip. */
export function NumberField(props: NumberFieldProps) {
  const field = useFieldContext();
  const { value, onValueChange, min, max, unit, chip, disabled, readOnly } = props;
  const step = props.step ?? 1;
  const decimals = decimalsOf(step);
  const [draft, setDraft] = useState<string | null>(null);

  const shown = draft ?? (value === undefined ? '' : String(value));
  const draftValue = draft === null ? value : parse(draft);
  const outOfRange =
    draftValue !== undefined &&
    ((min !== undefined && draftValue < min) || (max !== undefined && draftValue > max));
  const unparsable = draft !== null && draft.trim() !== '' && parse(draft) === undefined;
  const invalid = props.invalid ?? (field.invalid || outOfRange || unparsable);

  const commit = (next: number | undefined): void => {
    setDraft(null);
    onValueChange(next === undefined ? undefined : clamp(next, min, max));
  };

  const stepBy = (direction: 1 | -1, multiplier: number): void => {
    const base = draftValue ?? min ?? 0;
    const next = Number((base + direction * step * multiplier).toFixed(decimals));
    commit(next);
  };

  const onKeyDown = (event: KeyboardEvent): void => {
    if (disabled || readOnly) return;
    const mult = event.shiftKey ? 10 : 1;
    switch (event.key) {
      case 'ArrowUp':
        event.preventDefault();
        stepBy(1, mult);
        break;
      case 'ArrowDown':
        event.preventDefault();
        stepBy(-1, mult);
        break;
      case 'PageUp':
        event.preventDefault();
        stepBy(1, 10);
        break;
      case 'PageDown':
        event.preventDefault();
        stepBy(-1, 10);
        break;
      case 'Enter':
        commit(parse(shown));
        break;
      default:
    }
  };

  return (
    <div
      class={cx(
        'flex h-control items-center rounded-sm border bg-raised focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-focus',
        invalid ? 'border-danger' : 'border-line-strong hover:border-accent',
        disabled && 'opacity-50',
      )}
    >
      <input
        id={props.id ?? field.id}
        type="text"
        inputMode="decimal"
        role="spinbutton"
        aria-label={props['aria-label']}
        aria-describedby={field.describedBy}
        aria-invalid={invalid ? 'true' : undefined}
        aria-valuenow={draftValue}
        aria-valuemin={min}
        aria-valuemax={max}
        value={shown}
        placeholder={props.placeholder}
        disabled={disabled}
        readOnly={readOnly}
        required={field.required}
        onInput={(e) => {
          const text = e.currentTarget.value;
          setDraft(text);
          const n = parse(text);
          if (n !== undefined && (min === undefined || n >= min) && (max === undefined || n <= max))
            onValueChange(n);
        }}
        onBlur={() => {
          if (draft !== null) commit(parse(draft));
        }}
        onKeyDown={onKeyDown}
        class="min-w-0 flex-1 bg-transparent px-2 text-right font-mono text-mono text-fg tabular-nums outline-none placeholder:text-faint"
      />
      {unit ? <span class="pr-2 font-mono text-mono-small text-faint">{unit}</span> : null}
      {chip ? <span class="pr-2">{chip}</span> : null}
    </div>
  );
}
