import type { ExtraMeleeDamageDto } from 'rimstudio-ipc-types';
import { Button, IconButton } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { getAt, type Pointer } from '../../model/pointer';
import { useFieldEnv } from './fieldEnv';
import { NumberFieldRow } from './NumberFieldRow';
import { TextFieldRow } from './TextFieldRow';

export interface ExtraDamageRowsProps {
  /** Where the list of extra damages lives, for example /tools/0/extraMeleeDamages. */
  pointer: Pointer;
  label: string;
  addLabel: string;
}

/** The extra damages of a tool or of its surprise attack: a damage def with an optional amount and chance. */
export function ExtraDamageRows({ pointer, label, addLabel }: ExtraDamageRowsProps) {
  const env = useFieldEnv();
  const raw = getAt(env.spec, pointer);
  const rows = Array.isArray(raw) ? (raw as ExtraMeleeDamageDto[]) : [];
  return (
    <div data-field={pointer} class="flex flex-col gap-2">
      <h4 class="text-small font-semibold text-muted">{label}</h4>
      {rows.map((row, index) => (
        <div key={index} class="grid grid-cols-[1fr_auto] items-end gap-2">
          <div class="grid grid-cols-1 gap-2 sm:grid-cols-3">
            <TextFieldRow
              pointer={`${pointer}/${index}/def`}
              label={t('designer.extraDamage.def')}
              keepEmpty
            />
            <NumberFieldRow
              def={{
                pointer: `${pointer}/${index}/amount`,
                label: 'designer.extraDamage.amount',
                step: 1,
                plain: true,
              }}
            />
            <NumberFieldRow
              def={{
                pointer: `${pointer}/${index}/chance`,
                label: 'designer.extraDamage.chance',
                step: 0.05,
                plain: true,
              }}
            />
          </div>
          <IconButton
            icon="trash"
            variant="danger"
            label={t('designer.extraDamage.remove', { name: row.def || String(index + 1) })}
            onClick={() =>
              env.setField(rows.length === 1 ? pointer : `${pointer}/${index}`, undefined)
            }
          />
        </div>
      ))}
      <div>
        <Button
          size="sm"
          icon="plus"
          onClick={() => env.setField(`${pointer}/${rows.length}`, { def: '' })}
        >
          {addLabel}
        </Button>
      </div>
    </div>
  );
}
