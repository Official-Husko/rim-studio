import { useEffect, useState } from 'preact/hooks';
import { FormField, TextField } from 'rimstudio-ui';
import { diagnosticsFor, joinList, splitList } from '../../model/draft';
import { getAt, type Pointer } from '../../model/pointer';
import { DiagnosticNotes } from './DiagnosticNotes';
import { useFieldEnv } from './fieldEnv';

export interface TextFieldRowProps {
  pointer: Pointer;
  label: string;
  /** Pointer used to match diagnostics when it differs from the value pointer (/parent). */
  diagnosticPointer?: Pointer;
  /** A comma separated list stored as an array of strings. */
  list?: boolean;
  multiline?: boolean;
  /** Keep an empty text as an empty string instead of removing the value (identity fields). */
  keepEmpty?: boolean;
  help?: string;
  placeholder?: string;
}

function readText(value: unknown, list: boolean): string {
  if (list) return joinList(Array.isArray(value) ? (value as string[]) : undefined);
  return typeof value === 'string' ? value : '';
}

/**
 * A text or list field. A list is edited as text and committed on blur, so a trailing comma
 * survives while the user types. An empty text removes the value.
 */
export function TextFieldRow({
  pointer,
  label,
  diagnosticPointer,
  list = false,
  multiline,
  keepEmpty,
  help,
  placeholder,
}: TextFieldRowProps) {
  const env = useFieldEnv();
  const stored = readText(getAt(env.spec, pointer), list);
  const [text, setText] = useState(stored);
  useEffect(() => setText(stored), [stored]);
  const diagnostics = diagnosticsFor(env.diagnostics, diagnosticPointer ?? pointer);
  const errors = diagnostics.filter((d) => d.severity === 'error').map((d) => d.message);

  const commit = (value: string): void => {
    if (list) {
      const items = splitList(value);
      env.setField(pointer, items.length === 0 ? undefined : items);
    } else {
      env.setField(pointer, value === '' && !keepEmpty ? undefined : value);
    }
  };

  return (
    <div data-field={diagnosticPointer ?? pointer} class="flex min-w-0 flex-col gap-1">
      <FormField label={label} {...(help ? { help } : {})} error={errors.join(' ') || undefined}>
        <TextField
          value={text}
          multiline={multiline}
          {...(placeholder ? { placeholder } : {})}
          onValueChange={(value) => {
            setText(value);
            if (!list) commit(value);
          }}
          onBlur={() => {
            if (list && text !== stored) commit(text);
          }}
          onKeyDown={(event) => {
            if (list && event.key === 'Enter') commit(text);
          }}
        />
      </FormField>
      <DiagnosticNotes diagnostics={diagnostics} />
    </div>
  );
}
