import type { CeAmmoSuggestedFieldDto, CeRatingDto, DiagnosticDto } from 'rimstudio-ipc-types';
import { Badge, Button, type BadgeTone } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';

const RATING_TONE: Record<CeRatingDto, BadgeTone> = {
  reliable: 'success',
  rough: 'info',
  unreliable: 'warning',
  unmeasured: 'neutral',
};

/** The words of a rating. */
export function ratingText(rating: CeRatingDto): string {
  switch (rating) {
    case 'reliable':
      return t('ammo.rating.reliable');
    case 'rough':
      return t('ammo.rating.rough');
    case 'unreliable':
      return t('ammo.rating.unreliable');
    default:
      return t('ammo.rating.unmeasured');
  }
}

export interface FieldNotesProps {
  /** The suggestion of the backend for this field. */
  suggestion?: CeAmmoSuggestedFieldDto | undefined;
  /** What the field holds now, to hide the suggestion when it is already in. */
  current: number | string | undefined;
  unit?: string | undefined;
  onUse: () => void;
  /** The diagnostics of the field; an error is shown by the form field itself. */
  diagnostics: readonly DiagnosticDto[];
}

function suggestedText(s: CeAmmoSuggestedFieldDto, unit: string | undefined): string {
  const value = s.value !== undefined ? formatNumber(s.value, 3) : (s.text ?? '');
  return unit && s.value !== undefined ? `${value} ${unit}` : value;
}

/** Under a field: the backend's suggestion with its source and rating, and the warnings about the value. */
export function FieldNotes({ suggestion, current, unit, onUse, diagnostics }: FieldNotesProps) {
  const same =
    suggestion !== undefined &&
    (suggestion.value !== undefined ? suggestion.value === current : suggestion.text === current);
  const notes = diagnostics.filter((d) => d.severity !== 'error');
  return (
    <>
      {suggestion && !same ? (
        <div class="flex flex-wrap items-center gap-2 text-small text-muted" data-ammo-suggestion>
          <span>
            {t('ammo.suggestion.line', {
              value: suggestedText(suggestion, unit),
              source: sourceText(suggestion),
            })}
          </span>
          <Badge tone={RATING_TONE[suggestion.rating]}>{ratingText(suggestion.rating)}</Badge>
          {suggestion.range ? (
            <span class="font-mono text-mono-small">
              {t('ammo.suggestion.range', {
                low: formatNumber(suggestion.range[0], 3),
                high: formatNumber(suggestion.range[1], 3),
              })}
            </span>
          ) : null}
          <Button size="sm" onClick={onUse}>
            {t('ammo.suggestion.use')}
          </Button>
        </div>
      ) : null}
      {notes.map((d, i) => (
        <p key={`${d.code}-${i}`} class="m-0 text-small text-warning" data-ammo-note={d.code}>
          {d.message}
        </p>
      ))}
    </>
  );
}

function sourceText(s: CeAmmoSuggestedFieldDto): string {
  if (s.source === 'copied') return t('ammo.suggestion.copied', { from: s.from[0] ?? '' });
  if (s.source === 'hint') return t('ammo.suggestion.hint');
  return t('ammo.suggestion.nearest', { n: s.n });
}
