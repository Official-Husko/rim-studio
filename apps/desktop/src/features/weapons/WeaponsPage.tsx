import { EmptyState } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

/** Placeholder page; a later task builds the real one. */
export default function WeaponsPage() {
  return (
    <div class="p-6">
      <EmptyState icon="file" title={t('nav.weapons')} description={t('page.placeholder.body')} />
    </div>
  );
}
