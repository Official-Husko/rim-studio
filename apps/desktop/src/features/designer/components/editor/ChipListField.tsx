import { useState } from 'preact/hooks';
import { Button, Chip, FormField, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { diagnosticsFor, splitList } from '../../model/draft';
import { getAt, type Pointer } from '../../model/pointer';
import { DiagnosticNotes } from './DiagnosticNotes';
import { useFieldEnv } from './fieldEnv';

export interface ChipListFieldProps {
  pointer: Pointer;
  label: string;
  help?: string;
  /** Keep an empty list as [] instead of removing the value (a list the contract requires). */
  keepEmpty?: boolean;
}

/** A list of names shown as chips, with a field to add more. Several names can be pasted at once. */
export function ChipListField({ pointer, label, help, keepEmpty }: ChipListFieldProps) {
  const env = useFieldEnv();
  const raw = getAt(env.spec, pointer);
  const items = Array.isArray(raw) ? (raw as string[]) : [];
  const [text, setText] = useState('');
  const diagnostics = diagnosticsFor(env.diagnostics, pointer);
  const errors = diagnostics.filter((d) => d.severity === 'error').map((d) => d.message);

  const write = (next: string[]): void => {
    env.setField(pointer, next.length === 0 && !keepEmpty ? undefined : next);
  };
  const add = (): void => {
    const fresh = splitList(text).filter((name) => !items.includes(name));
    setText('');
    if (fresh.length > 0) write([...items, ...new Set(fresh)]);
  };

  return (
    <div data-field={pointer} class="flex min-w-0 flex-col gap-1.5">
      <FormField label={label} {...(help ? { help } : {})} error={errors.join(' ') || undefined}>
        <div class="flex items-center gap-2">
          <div class="min-w-0 flex-1">
            <TextField
              value={text}
              onValueChange={setText}
              placeholder={t('designer.chips.placeholder')}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault();
                  add();
                }
              }}
            />
          </div>
          <Button size="sm" icon="plus" disabled={text.trim() === ''} onClick={add}>
            {t('designer.chips.add')}
          </Button>
        </div>
      </FormField>
      {items.length > 0 ? (
        <ul class="flex flex-wrap gap-1.5">
          {items.map((name) => (
            <li key={name}>
              <Chip
                removeLabel={t('designer.chips.remove', { name })}
                onRemove={() => write(items.filter((item) => item !== name))}
              >
                {name}
              </Chip>
            </li>
          ))}
        </ul>
      ) : null}
      <DiagnosticNotes diagnostics={diagnostics} />
    </div>
  );
}
