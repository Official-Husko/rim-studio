import { useState } from 'preact/hooks';
import { Button, Chip, FormField, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

export interface StringListFieldProps {
  label: string;
  help?: string;
  values: readonly string[];
  onChange: (values: string[]) => void;
  placeholder?: string;
}

/** A list of names as chips with a box to add one. The same name is never added twice. */
export function StringListField({
  label,
  help,
  values,
  onChange,
  placeholder,
}: StringListFieldProps) {
  const [draft, setDraft] = useState('');
  const add = (): void => {
    const name = draft.trim();
    if (name === '' || values.includes(name)) {
      setDraft('');
      return;
    }
    onChange([...values, name]);
    setDraft('');
  };
  return (
    <FormField label={label} {...(help ? { help } : {})}>
      <div class="flex flex-col gap-2">
        {values.length > 0 ? (
          <ul class="m-0 flex list-none flex-wrap gap-1 p-0">
            {values.map((name) => (
              <li key={name}>
                <Chip
                  kind="typed"
                  removeLabel={t('ceblock.remove', { name })}
                  onRemove={() => onChange(values.filter((v) => v !== name))}
                >
                  {name}
                </Chip>
              </li>
            ))}
          </ul>
        ) : null}
        <div class="flex items-center gap-2">
          <div class="min-w-0 flex-1">
            <TextField
              aria-label={label}
              value={draft}
              onValueChange={setDraft}
              {...(placeholder ? { placeholder } : {})}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault();
                  add();
                }
              }}
            />
          </div>
          <Button size="sm" variant="secondary" disabled={draft.trim() === ''} onClick={add}>
            {t('ceblock.add')}
          </Button>
        </div>
      </div>
    </FormField>
  );
}
