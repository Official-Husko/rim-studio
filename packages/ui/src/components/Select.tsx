import { cx } from '../cx';
import { useFieldContext } from './fieldContext';

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
  /** Options with the same group text are grouped under it. */
  group?: string;
}

export interface SelectProps {
  value: string | undefined;
  onValueChange: (value: string) => void;
  options: SelectOption[];
  'aria-label'?: string;
  id?: string;
  /** Shown as a disabled first entry while nothing is chosen. */
  placeholder?: string;
  invalid?: boolean;
  disabled?: boolean;
}

/** A styled native select: the browser supplies the list and the keyboard model. */
export function Select({
  value,
  onValueChange,
  options,
  placeholder,
  disabled,
  ...rest
}: SelectProps) {
  const field = useFieldContext();
  const invalid = rest.invalid ?? field.invalid;
  const groups = [...new Set(options.map((o) => o.group).filter((g): g is string => Boolean(g)))];
  const render = (o: SelectOption) => (
    <option key={o.value} value={o.value} disabled={o.disabled}>
      {o.label}
    </option>
  );
  return (
    <select
      id={rest.id ?? field.id}
      aria-label={rest['aria-label']}
      aria-describedby={field.describedBy}
      aria-invalid={invalid ? 'true' : undefined}
      required={field.required}
      disabled={disabled}
      value={value ?? ''}
      onChange={(e) => onValueChange(e.currentTarget.value)}
      class={cx(
        'h-control rounded-sm border bg-raised px-2 text-body text-fg',
        invalid ? 'border-danger' : 'border-line-strong hover:border-accent',
        'disabled:cursor-not-allowed disabled:opacity-50',
      )}
    >
      {placeholder ? (
        <option value="" disabled>
          {placeholder}
        </option>
      ) : null}
      {options.filter((o) => !o.group).map(render)}
      {groups.map((g) => (
        <optgroup key={g} label={g}>
          {options.filter((o) => o.group === g).map(render)}
        </optgroup>
      ))}
    </select>
  );
}
