import type { CeAttachmentLinkDto, CeStatEntryDto } from 'rimstudio-ipc-types';
import { Button, FormField, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { StatEntryList } from './StatEntryList';

export interface AttachmentLinkRowProps {
  link: CeAttachmentLinkDto;
  onChange: (link: CeAttachmentLinkDto) => void;
  onRemove: () => void;
}

type StatList = 'statOffsets' | 'statMultipliers' | 'statReplacers';

/** One attachment a platform accepts: the definition, how it is drawn and the stats it changes. */
export function AttachmentLinkRow({ link, onChange, onRemove }: AttachmentLinkRowProps) {
  const text = (key: 'drawScale' | 'drawOffset', value: string): void => {
    const next = { ...link };
    if (value === '') delete next[key];
    else next[key] = value;
    onChange(next);
  };
  const list = (key: StatList, entries: CeStatEntryDto[]): void => {
    const next = { ...link };
    if (entries.length === 0) delete next[key];
    else next[key] = entries;
    onChange(next);
  };
  const name = link.attachment === '' ? t('ceblock.link.unnamed') : link.attachment;
  return (
    <div
      class="flex flex-col gap-3 rounded-sm border border-line p-3"
      role="group"
      aria-label={name}
    >
      <div class="flex items-end gap-2">
        <div class="min-w-0 flex-1">
          <FormField label={t('ceblock.link.attachment')} help={t('ceblock.link.attachmentHelp')}>
            <TextField
              aria-label={t('ceblock.link.attachment')}
              value={link.attachment}
              onValueChange={(attachment) => onChange({ ...link, attachment })}
            />
          </FormField>
        </div>
        <Button size="sm" variant="secondary" onClick={onRemove}>
          {t('ceblock.link.remove')}
        </Button>
      </div>
      <div class="grid grid-cols-2 gap-3">
        <FormField label={t('ceblock.link.drawScale')} help={t('ceblock.link.vectorHelp')}>
          <TextField
            aria-label={t('ceblock.link.drawScale')}
            value={link.drawScale ?? ''}
            placeholder="(0.5,0.5)"
            onValueChange={(v) => text('drawScale', v)}
          />
        </FormField>
        <FormField label={t('ceblock.link.drawOffset')}>
          <TextField
            aria-label={t('ceblock.link.drawOffset')}
            value={link.drawOffset ?? ''}
            placeholder="(0,0.1)"
            onValueChange={(v) => text('drawOffset', v)}
          />
        </FormField>
      </div>
      <StatEntryList
        label={t('ceblock.link.offsets')}
        entries={link.statOffsets ?? []}
        onChange={(e) => list('statOffsets', e)}
      />
      <StatEntryList
        label={t('ceblock.link.multipliers')}
        entries={link.statMultipliers ?? []}
        onChange={(e) => list('statMultipliers', e)}
      />
      <StatEntryList
        label={t('ceblock.link.replacers')}
        entries={link.statReplacers ?? []}
        onChange={(e) => list('statReplacers', e)}
      />
    </div>
  );
}
