import { Button } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { AssetStore } from '../../asset-store';
import { getAt, type Pointer } from '../../model/pointer';
import { useFieldEnv } from './fieldEnv';
import { SoundPicker } from './SoundPicker';
import { TextFieldRow } from './TextFieldRow';

export interface SoundNameRowProps {
  pointer: Pointer;
  label: string;
  help?: string;
  assets: AssetStore;
  /** True while this row's list of game sounds is open. */
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** A sound definition name: typed, or chosen from the searchable list of the game's sounds. */
export function SoundNameRow({
  pointer,
  label,
  help,
  assets,
  open,
  onOpenChange,
}: SoundNameRowProps) {
  const env = useFieldEnv();
  const value = getAt(env.spec, pointer);
  return (
    <div class="flex min-w-0 flex-col gap-2">
      <div class="flex items-end gap-2">
        <div class="min-w-0 flex-1">
          <TextFieldRow pointer={pointer} label={label} {...(help ? { help } : {})} />
        </div>
        <Button
          size="sm"
          aria-expanded={open}
          aria-label={t('designer.sounds.browseLabel', { field: label })}
          onClick={() => onOpenChange(!open)}
        >
          {t('designer.sounds.browse')}
        </Button>
      </div>
      {open ? (
        <SoundPicker
          assets={assets}
          label={label}
          value={typeof value === 'string' ? value : undefined}
          onChoose={(name) => {
            env.setField(pointer, name);
            onOpenChange(false);
          }}
          onClose={() => onOpenChange(false)}
        />
      ) : null}
    </div>
  );
}
