import type { CeStatEntryDto } from 'rimstudio-ipc-types';
import { Button, IconButton, NumberField, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { newStat, replaceAt } from './blockModel';

export interface StatEntryListProps {
  /** The name of the list, for example "Stat offsets". */
  label: string;
  entries: readonly CeStatEntryDto[];
  onChange: (entries: CeStatEntryDto[]) => void;
}

/** A list of stat names with a number each, as an attachment link carries them. */
export function StatEntryList({ label, entries, onChange }: StatEntryListProps) {
  return (
    <div class="flex flex-col gap-1" role="group" aria-label={label}>
      <span class="text-small text-muted">{label}</span>
      {entries.map((entry, index) => (
        <div key={index} class="flex items-center gap-2">
          <div class="min-w-0 flex-1">
            <TextField
              aria-label={t('ceblock.stat.name', { label })}
              value={entry.stat}
              placeholder={t('ceblock.stat.placeholder')}
              onValueChange={(stat) => onChange(replaceAt(entries, index, { ...entry, stat }))}
            />
          </div>
          <div class="w-28">
            <NumberField
              aria-label={t('ceblock.stat.value', { label })}
              value={entry.value}
              step={0.01}
              onValueChange={(value) =>
                onChange(replaceAt(entries, index, { ...entry, value: value ?? 0 }))
              }
            />
          </div>
          <IconButton
            icon="trash"
            label={t('ceblock.stat.remove', { label })}
            onClick={() => onChange(replaceAt(entries, index, undefined))}
          />
        </div>
      ))}
      <div class="self-start">
        <Button size="sm" variant="secondary" onClick={() => onChange([...entries, newStat()])}>
          {t('ceblock.stat.add')}
        </Button>
      </div>
    </div>
  );
}
