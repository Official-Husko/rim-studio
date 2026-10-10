import { Banner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

export interface CloneNotesProps {
  notes: readonly string[];
  onDismiss: () => void;
  /** Accessible names of the group and of the dismiss button, for notes that are not about a clone. */
  label?: string;
  dismissLabel?: string;
}

/**
 * What the backend said when it made the draft: for a clone what is shared with the source and what
 * is missing, for a weapon loaded from the project what the designer keeps as it is.
 */
export function CloneNotes({ notes, onDismiss, label, dismissLabel }: CloneNotesProps) {
  if (notes.length === 0) return null;
  return (
    <div class="flex flex-col gap-2" aria-label={label ?? t('designer.cloneNotes.label')}>
      {notes.map((note, index) => (
        <Banner
          key={note}
          tone="info"
          dismissLabel={dismissLabel ?? t('designer.cloneNotes.dismiss')}
          {...(index === 0 ? { onDismiss } : {})}
        >
          {note}
        </Banner>
      ))}
    </div>
  );
}
