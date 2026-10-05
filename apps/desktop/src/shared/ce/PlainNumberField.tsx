import { FormField, NumberField } from 'rimstudio-ui';

export interface PlainNumberFieldProps {
  label: string;
  help?: string;
  value: number | undefined;
  onChange: (value: number | undefined) => void;
  whole?: boolean;
  step?: number;
  unit?: string;
  placeholder?: string;
}

/** A plain number of the block with no source; clearing the box removes it. */
export function PlainNumberField({
  label,
  help,
  value,
  onChange,
  whole,
  step,
  unit,
  placeholder,
}: PlainNumberFieldProps) {
  return (
    <FormField label={label} {...(help ? { help } : {})}>
      <div class="w-32">
        <NumberField
          aria-label={label}
          value={value}
          onValueChange={(n) => onChange(n === undefined ? undefined : whole ? Math.round(n) : n)}
          step={step ?? (whole ? 1 : 0.01)}
          {...(unit ? { unit } : {})}
          {...(placeholder ? { placeholder } : {})}
        />
      </div>
    </FormField>
  );
}
