import { EmptyState } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

/** Placeholder page; a later task builds the real one. */
export default function ProjectPage() {
  return (
    <div class="p-6">
      <EmptyState icon="file" title={t('nav.project')} description={t('page.placeholder.body')} />
    </div>
  );
}
