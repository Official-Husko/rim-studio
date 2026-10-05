import { FormField, TextField } from 'rimstudio-ui';
import type { AboutTextFieldDto } from 'rimstudio-ipc-types';
import { FieldFindings } from './FieldFindings';
import { findingsFor, textOf } from './aboutModel';
import { aboutDraft, aboutModel, findings, setText } from './aboutStore';

export interface TextRowProps {
  field: AboutTextFieldDto;
  label: string;
  help?: string;
  placeholder?: string;
  required?: boolean;
  multiline?: boolean;
  rows?: number;
}

/** One text field of About.xml with the backend's findings under it. */
export function TextRow({
  field,
  label,
  help,
  placeholder,
  required,
  multiline,
  rows,
}: TextRowProps) {
  const about = aboutModel.value;
  if (!about) return null;
  const mine = findingsFor(findings.value, field);
  const error = mine.find((item) => item.severity === 'error');
  const rest = mine.filter((item) => item !== error);
  return (
    <div class="flex flex-col gap-1">
      <FormField
        label={label}
        required={required}
        {...(help ? { help } : {})}
        {...(error ? { error: error.message } : {})}
      >
        <TextField
          value={textOf(about, aboutDraft.value, field)}
          onValueChange={(value) => setText(field, value)}
          disabled={!about.editable}
          {...(placeholder ? { placeholder } : {})}
          {...(multiline ? { multiline: true, rows: rows ?? 4 } : {})}
        />
      </FormField>
      <FieldFindings items={rest} />
    </div>
  );
}
