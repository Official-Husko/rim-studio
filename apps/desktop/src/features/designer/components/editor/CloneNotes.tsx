import { Banner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

export interface CloneNotesProps {
  notes: readonly string[];
  onDismiss: () => void;
}

/** What the backend said when it made the clone: what is shared with the source and what is missing. */
export function CloneNotes({ notes, onDismiss }: CloneNotesProps) {
  if (notes.length === 0) return null;
  return (
    <div class="flex flex-col gap-2" aria-label={t('designer.cloneNotes.label')}>
      {notes.map((note, index) => (
        <Banner
          key={note}
          tone="info"
          dismissLabel={t('designer.cloneNotes.dismiss')}
          {...(index === 0 ? { onDismiss } : {})}
        >
          {note}
        </Banner>
      ))}
    </div>
  );
}
