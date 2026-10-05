import { Banner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { UndoFlow } from './fixStore';

/** The outcome of an undo: what went back, or why it could not. */
export function FixUndoNotice({ undo }: { undo: UndoFlow | undefined }) {
  if (undo?.result) {
    return (
      <Banner tone="success" title={t('project.fix.undo.done')}>
        {t('project.fix.undo.summary', {
          moved: undo.result.movedBack.length,
          restored: undo.result.restored.length,
          folders: undo.result.removedFolders.length,
        })}
      </Banner>
    );
  }
  if (undo?.error) {
    return (
      <Banner tone="error" title={t('project.fix.undo.error')}>
        {undo.error.message}
      </Banner>
    );
  }
  return null;
}
