import type { SourcedDto } from 'rimstudio-ipc-types';
import { FormField, NumberField } from 'rimstudio-ui';
import { typed, typedInt } from './blockModel';

export interface SourcedNumberFieldProps {
  label: string;
  help?: string;
  value: SourcedDto<number> | undefined;
  onChange: (value: SourcedDto<number> | undefined) => void;
  step?: number;
  unit?: string;
  whole?: boolean;
  min?: number;
  placeholder?: string;
}

/** A number of the block with its source: typing sets it as typed, clearing removes it. */
export function SourcedNumberField({
  label,
  help,
  value,
  onChange,
  step,
  unit,
  whole,
  min,
  placeholder,
}: SourcedNumberFieldProps) {
  return (
    <FormField label={label} {...(help ? { help } : {})}>
      <div class="w-40">
        <NumberField
          aria-label={label}
          value={value?.value}
          onValueChange={(n) =>
            onChange(n === undefined ? undefined : whole ? typedInt(n) : typed(n))
          }
          step={step ?? (whole ? 1 : 0.01)}
          {...(min !== undefined ? { min } : {})}
          {...(unit ? { unit } : {})}
          {...(placeholder ? { placeholder } : {})}
        />
      </div>
    </FormField>
  );
}
