import { useEffect, useState } from 'preact/hooks';
import { Banner, Button, Spinner, TextField } from 'rimstudio-ui';
import type { LibraryModHitDto, LibraryModSearchDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { normalizeError } from '~/shared/ipc';
import { searchLibrary } from '../modApi';

/** How long typing must pause before the library is searched. */
export const SEARCH_DELAY_MS = 250;

export interface ModPickerProps {
  /** Called with the mod the person chose. */
  onPick: (hit: LibraryModHitDto) => void;
  /** Accessible name of the search field. */
  label: string;
  /** Package ids to mark as already added. */
  taken?: readonly string[];
}

/** A searchable list of the mods of the last library scan: name, package id and where the mod lives. */
export function ModPicker({ onPick, label, taken = [] }: ModPickerProps) {
  const [query, setQuery] = useState('');
  const [answer, setAnswer] = useState<LibraryModSearchDto | undefined>(undefined);
  const [failed, setFailed] = useState<string | undefined>(undefined);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (query.trim() === '') {
      setAnswer(undefined);
      setFailed(undefined);
      setBusy(false);
      return undefined;
    }
    let live = true;
    setBusy(true);
    const timer = setTimeout(() => {
      searchLibrary(query.trim(), 12).then(
        (found) => {
          if (!live) return;
          setAnswer(found);
          setFailed(undefined);
          setBusy(false);
        },
        (thrown) => {
          if (!live) return;
          setFailed(normalizeError(thrown).message);
          setBusy(false);
        },
      );
    }, SEARCH_DELAY_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [query]);

  const takenIds = new Set(taken.map((id) => id.toLowerCase()));
  return (
    <div class="flex flex-col gap-2 border border-line-subtle bg-raised p-3">
      <TextField
        type="search"
        aria-label={label}
        placeholder={t('project.basics.picker.placeholder')}
        value={query}
        onValueChange={setQuery}
        suffix={busy ? <Spinner label={t('project.basics.picker.searching')} /> : undefined}
      />
      {failed ? <Banner tone="error">{failed}</Banner> : null}
      {answer && !answer.scanned ? (
        <Banner tone="info">{answer.hint ?? t('project.basics.picker.unscanned')}</Banner>
      ) : null}
      {answer?.scanned && answer.hits.length === 0 ? (
        <p class="m-0 text-small text-muted">{t('project.basics.picker.none', { query })}</p>
      ) : null}
      {answer?.scanned && answer.hits.length > 0 ? (
        <ul
          class="m-0 max-h-60 list-none divide-y divide-line-subtle overflow-auto p-0"
          aria-label={t('project.basics.picker.results')}
        >
          {answer.hits.map((hit) => (
            <li key={`${hit.packageId}:${hit.path}`} class="flex items-center gap-3 py-1">
              <span class="min-w-0 flex-1">
                <span class="block truncate">{hit.name}</span>
                <span class="block truncate font-mono text-mono-small text-faint">
                  {hit.packageId}
                </span>
              </span>
              <Button
                size="sm"
                variant="secondary"
                aria-label={t('project.basics.picker.add', { name: hit.name })}
                disabled={takenIds.has(hit.packageId.toLowerCase())}
                onClick={() => onPick(hit)}
              >
                {takenIds.has(hit.packageId.toLowerCase())
                  ? t('project.basics.picker.added')
                  : t('project.basics.picker.pick')}
              </Button>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
