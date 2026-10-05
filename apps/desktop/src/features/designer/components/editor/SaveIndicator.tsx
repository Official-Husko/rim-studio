import { Badge, Spinner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { SaveState } from '../../editor-store';

export interface SaveIndicatorProps {
  state: SaveState;
}

/** The autosave state of the open draft, always in words. */
export function SaveIndicator({ state }: SaveIndicatorProps) {
  switch (state) {
    case 'saving':
      return (
        <span role="status" class="inline-flex items-center gap-1 text-small text-muted">
          <Spinner size="sm" label={t('designer.save.saving')} />
          {t('designer.save.saving')}
        </span>
      );
    case 'dirty':
      return <Badge tone="warning">{t('designer.save.dirty')}</Badge>;
    case 'saved':
      return <Badge tone="success">{t('designer.save.saved')}</Badge>;
    case 'error':
      return <Badge tone="danger">{t('designer.save.error')}</Badge>;
    default:
      return null;
  }
}
