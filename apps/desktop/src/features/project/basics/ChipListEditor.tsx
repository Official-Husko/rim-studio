import { useState } from 'preact/hooks';
import { Button, Chip, FormField, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { FieldFindings } from './FieldFindings';
import { ModPicker } from './ModPicker';
import type { DiagnosticDto } from 'rimstudio-ipc-types';

export interface ChipListEditorProps {
  label: string;
  help?: string;
  items: readonly string[];
  onChange: (items: string[]) => void;
  /** Offer the library search to fill the list with package ids. */
  library?: boolean;
  placeholder?: string;
  disabled?: boolean;
  findings?: readonly DiagnosticDto[];
}

/** A list of short texts shown as removable chips, with a field to add one and optionally the library picker. */
export function ChipListEditor({
  label,
  help,
  items,
  onChange,
  library,
  placeholder,
  disabled,
  findings = [],
}: ChipListEditorProps) {
  const [text, setText] = useState('');
  const [picking, setPicking] = useState(false);

  const add = (value: string): void => {
    const next = value.trim();
    if (next === '' || items.some((item) => item.toLowerCase() === next.toLowerCase())) return;
    onChange([...items, next]);
  };

  return (
    <div class="flex flex-col gap-2">
      <FormField label={label} {...(help ? { help } : {})}>
        <TextField
          value={text}
          onValueChange={setText}
          disabled={disabled}
          {...(placeholder ? { placeholder } : {})}
          onKeyDown={(event) => {
            if (event.key !== 'Enter') return;
            event.preventDefault();
            add(text);
            setText('');
          }}
          suffix={
            <Button
              size="sm"
              variant="ghost"
              icon="plus"
              disabled={disabled || text.trim() === ''}
              aria-label={t('project.basics.chips.add', { name: label })}
              onClick={() => {
                add(text);
                setText('');
              }}
            >
              {t('project.basics.chips.addShort')}
            </Button>
          }
        />
      </FormField>
      {items.length > 0 ? (
        <ul class="m-0 flex list-none flex-wrap gap-1 p-0" aria-label={label}>
          {items.map((item) => (
            <li key={item}>
              <Chip
                removeLabel={t('project.basics.chips.remove', { name: item })}
                {...(disabled ? {} : { onRemove: () => onChange(items.filter((x) => x !== item)) })}
              >
                {item}
              </Chip>
            </li>
          ))}
        </ul>
      ) : null}
      {library ? (
        <div class="flex flex-col gap-2">
          <div>
            <Button
              size="sm"
              variant="secondary"
              icon="search"
              disabled={disabled}
              aria-expanded={picking}
              onClick={() => setPicking(!picking)}
            >
              {t('project.basics.chips.library', { name: label })}
            </Button>
          </div>
          {picking ? (
            <ModPicker
              label={t('project.basics.chips.search', { name: label })}
              taken={items}
              onPick={(hit) => add(hit.packageId)}
            />
          ) : null}
        </div>
      ) : null}
      <FieldFindings items={findings} />
    </div>
  );
}
