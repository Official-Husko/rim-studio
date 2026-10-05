import { useEffect, useState } from 'preact/hooks';
import { Banner, Button, Spinner, TextField } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { SOUND_PAGE } from '../../asset-api';
import type { AssetStore } from '../../asset-store';

/** Milliseconds after the last key before the search is asked. */
export const SEARCH_IDLE_MS = 200;

export interface SoundPickerProps {
  assets: AssetStore;
  /** The sound definition the field holds now. */
  value: string | undefined;
  /** What the picker is for, in the name of its controls. */
  label: string;
  onChoose: (defName: string) => void;
  onClose: () => void;
}

/** A searchable list of the sound definitions of the game and its expansions, shown by name. */
export function SoundPicker({ assets, value, label, onChoose, onClose }: SoundPickerProps) {
  const [text, setText] = useState('');
  useEffect(() => {
    const timer = setTimeout(() => void assets.searchSounds(text.trim()), SEARCH_IDLE_MS);
    return () => clearTimeout(timer);
  }, [assets, text]);

  const search = assets.sounds.value;
  return (
    <div class="flex min-w-0 flex-col gap-2 border border-line p-2">
      <TextField
        type="search"
        aria-label={t('designer.sounds.searchLabel', { field: label })}
        placeholder={t('designer.sounds.searchPlaceholder')}
        value={text}
        onValueChange={setText}
      />
      {search.status === 'failed' && search.error ? (
        <Banner tone="error" title={search.error.code}>
          {search.error.message}
        </Banner>
      ) : null}
      <ul
        aria-label={t('designer.sounds.resultsLabel', { field: label })}
        class="flex max-h-56 flex-col overflow-auto"
      >
        {search.rows.map((row) => (
          <li key={`${row.modId}-${row.defName}`}>
            <button
              type="button"
              aria-pressed={row.defName === value}
              onClick={() => onChoose(row.defName)}
              class="flex min-h-control w-full items-center justify-between gap-3 px-2 text-left text-body hover:bg-hover aria-pressed:font-semibold aria-pressed:text-accent"
            >
              <span class="truncate font-mono text-mono">{row.defName}</span>
              <span class="shrink-0 font-mono text-mono-small text-faint">{row.modId}</span>
            </button>
          </li>
        ))}
      </ul>
      <div class="flex flex-wrap items-center justify-between gap-2">
        <span class="flex items-center gap-2 text-small text-muted">
          {search.status === 'loading' ? <Spinner label={t('designer.sounds.searching')} /> : null}
          {search.status === 'ready'
            ? search.total > search.rows.length
              ? t('designer.sounds.showing', { shown: SOUND_PAGE, total: search.total })
              : tn('designer.sounds.matches', search.total)
            : null}
        </span>
        <Button size="sm" variant="ghost" onClick={onClose}>
          {t('designer.sounds.closeList')}
        </Button>
      </div>
    </div>
  );
}
