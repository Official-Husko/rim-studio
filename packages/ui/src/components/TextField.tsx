import type { ComponentChildren } from 'preact';
import { cx } from '../cx';
import { useFieldContext } from './fieldContext';

export interface TextFieldProps {
  value: string;
  onValueChange: (value: string) => void;
  /** Use FormField for a visible label, or pass an aria-label. */
  'aria-label'?: string;
  id?: string;
  placeholder?: string;
  type?: 'text' | 'search';
  multiline?: boolean;
  rows?: number;
  /** Content drawn inside the field before the text (an icon or a short label). */
  prefix?: ComponentChildren;
  /** Content drawn inside the field after the text (a unit or a chip). */
  suffix?: ComponentChildren;
  invalid?: boolean;
  disabled?: boolean;
  readOnly?: boolean;
  onBlur?: () => void;
  onKeyDown?: (event: KeyboardEvent) => void;
}

/** A single or multi line text input. */
export function TextField(props: TextFieldProps) {
  const field = useFieldContext();
  const {
    value,
    onValueChange,
    multiline,
    rows = 4,
    prefix,
    suffix,
    disabled,
    readOnly,
    placeholder,
  } = props;
  const invalid = props.invalid ?? field.invalid;
  const shared = {
    id: props.id ?? field.id,
    value,
    placeholder,
    disabled,
    readOnly,
    required: field.required,
    'aria-label': props['aria-label'],
    'aria-describedby': field.describedBy,
    'aria-invalid': invalid ? ('true' as const) : undefined,
    onBlur: props.onBlur,
    onKeyDown: props.onKeyDown,
    class:
      'min-w-0 flex-1 bg-transparent px-2 text-body text-fg outline-none placeholder:text-faint',
  };
  return (
    <div
      class={cx(
        'flex rounded-sm border bg-raised focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-focus',
        multiline ? 'items-start' : 'h-control items-center',
        invalid ? 'border-danger' : 'border-line-strong hover:border-accent',
        disabled && 'opacity-50',
      )}
    >
      {prefix ? <span class="pl-2 text-faint">{prefix}</span> : null}
      {multiline ? (
        <textarea
          {...shared}
          rows={rows}
          class={cx(shared.class, 'py-1.5')}
          onInput={(e) => onValueChange(e.currentTarget.value)}
        />
      ) : (
        <input
          {...shared}
          type={props.type ?? 'text'}
          onInput={(e) => onValueChange(e.currentTarget.value)}
        />
      )}
      {suffix ? <span class="pr-2 text-faint">{suffix}</span> : null}
    </div>
  );
}
