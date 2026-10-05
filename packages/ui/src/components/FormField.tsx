import type { ComponentChildren } from 'preact';
import { useId } from 'preact/hooks';
import { FieldContext } from './fieldContext';

export interface FormFieldProps {
  /** Visible label; always required so every control has a name. */
  label: string;
  help?: string;
  /** An error message; the field is marked invalid and the message is announced. */
  error?: string;
  required?: boolean;
  /** One control (TextField, NumberField, Select, Combobox). */
  children: ComponentChildren;
}

/** Label, control, help text and error, wired together with ids. */
export function FormField({ label, help, error, required, children }: FormFieldProps) {
  const base = useId();
  const id = `${base}-control`;
  const helpId = `${base}-help`;
  const errorId = `${base}-error`;
  const describedBy =
    [help ? helpId : '', error ? errorId : ''].filter(Boolean).join(' ') || undefined;
  return (
    <FieldContext.Provider value={{ id, describedBy, invalid: Boolean(error), required }}>
      <div class="flex flex-col gap-1">
        <label for={id} class="text-small font-semibold text-muted">
          {label}
          {required ? (
            <span aria-hidden="true" class="ml-1 text-accent">
              *
            </span>
          ) : null}
        </label>
        {children}
        {help ? (
          <p id={helpId} class="text-small text-faint">
            {help}
          </p>
        ) : null}
        {error ? (
          <p id={errorId} role="alert" class="text-small text-danger">
            {error}
          </p>
        ) : null}
      </div>
    </FieldContext.Provider>
  );
}
