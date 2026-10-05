import { t } from '~/shared/i18n';
import { SetTextField } from './SetTextField';
import type { CustomAmmoStore } from './store';

export interface IdentitySectionProps {
  store: CustomAmmoStore;
}

/** The name the def names come from and the caliber label. */
export function IdentitySection({ store }: IdentitySectionProps) {
  return (
    <div class="flex max-w-xl flex-col gap-4">
      <p class="m-0 text-small text-muted">{t('ammo.identity.intro')}</p>
      <SetTextField
        store={store}
        member="name"
        label={t('ammo.field.name')}
        help={t('ammo.field.name-help')}
        required
      />
      <SetTextField
        store={store}
        member="caliber"
        label={t('ammo.field.caliber')}
        help={t('ammo.field.caliber-help')}
        required
      />
      <SetTextField
        store={store}
        member="setLabel"
        label={t('ammo.field.set-label')}
        help={t('ammo.field.set-label-help')}
      />
    </div>
  );
}
