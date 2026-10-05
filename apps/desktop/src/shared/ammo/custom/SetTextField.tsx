import type { CustomAmmoDto } from 'rimstudio-ipc-types';
import { FormField, TextField } from 'rimstudio-ui';
import { AMMO_POINTER } from './model';
import type { CustomAmmoStore } from './store';

type TextKey = 'name' | 'caliber' | 'setLabel' | 'categoryParent' | 'categoryIcon';

export interface SetTextFieldProps {
  store: CustomAmmoStore;
  member: TextKey;
  label: string;
  help?: string;
  required?: boolean;
  placeholder?: string;
}

/** A text member of the caliber itself (name, caliber, labels) with the checks of the plan under it. */
export function SetTextField({
  store,
  member,
  label,
  help,
  required,
  placeholder,
}: SetTextFieldProps) {
  const pointer = `${AMMO_POINTER}/${member}`;
  const value = (store.custom.value as CustomAmmoDto)[member] ?? '';
  const mine = store.diagnostics.value.filter((d) => d.field === pointer);
  const error = mine.find((d) => d.severity === 'error')?.message;
  return (
    <div data-ammo-field={pointer} class="flex flex-col gap-1">
      <FormField
        label={label}
        {...(help ? { help } : {})}
        {...(error ? { error } : {})}
        {...(required ? { required: true } : {})}
      >
        <TextField
          value={value}
          invalid={error !== undefined}
          onValueChange={(v) => store.patch({ [member]: v })}
          {...(placeholder ? { placeholder } : {})}
        />
      </FormField>
      {mine
        .filter((d) => d.severity !== 'error')
        .map((d, i) => (
          <p key={`${d.code}-${i}`} class="m-0 text-small text-warning">
            {d.message}
          </p>
        ))}
    </div>
  );
}
