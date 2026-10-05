import { Banner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { AmmoTextField } from './fields';
import { typeTitle } from './model';
import type { CustomAmmoStore } from './store';

export interface ArtSectionProps {
  store: CustomAmmoStore;
}

/** The pictures and sounds of each type, as the paths the definitions will name. */
export function ArtSection({ store }: ArtSectionProps) {
  const types = store.custom.value.types;
  return (
    <div class="flex flex-col gap-4">
      <Banner tone="info" title={t('ammo.art.title')}>
        {t('ammo.art.note')}
      </Banner>
      {types.length === 0 ? (
        <p class="m-0 text-small text-muted">{t('ammo.art.no-types')}</p>
      ) : null}
      {types.map((type, index) => {
        const field = { store, index } as const;
        return (
          <section
            key={store.ids.value[index] ?? index}
            class="flex flex-col gap-3 border border-line p-3"
            aria-label={typeTitle(type, index)}
          >
            <h3 class="m-0 font-display text-label font-semibold tracking-label text-muted uppercase">
              {typeTitle(type, index)}
            </h3>
            <div class="grid grid-cols-[repeat(auto-fit,minmax(16rem,1fr))] gap-x-4 gap-y-3">
              <AmmoTextField
                {...field}
                pointer="/item/texPath"
                label={t('ammo.field.item-texture')}
                help={t('ammo.field.texture-help')}
              />
              <AmmoTextField
                {...field}
                pointer="/item/graphicClass"
                label={t('ammo.field.item-graphic')}
              />
              <AmmoTextField
                {...field}
                pointer="/item/drawSize"
                label={t('ammo.field.item-draw')}
              />
              <AmmoTextField
                {...field}
                pointer="/projectile/texPath"
                label={t('ammo.field.projectile-texture')}
                help={t('ammo.field.texture-help')}
              />
              <AmmoTextField
                {...field}
                pointer="/projectile/graphicClass"
                label={t('ammo.field.projectile-graphic')}
              />
              <AmmoTextField
                {...field}
                pointer="/projectile/drawSize"
                label={t('ammo.field.projectile-draw')}
              />
              <AmmoTextField
                {...field}
                pointer="/projectile/soundExplode"
                label={t('ammo.field.sound-explode')}
                help={t('ammo.field.sound-help')}
              />
              <AmmoTextField
                {...field}
                pointer="/projectile/soundAmbient"
                label={t('ammo.field.sound-ambient')}
              />
              <AmmoTextField
                {...field}
                pointer="/projectile/soundHitThickRoof"
                label={t('ammo.field.sound-roof')}
              />
              <AmmoTextField
                {...field}
                pointer="/projectile/soundImpactAnticipate"
                label={t('ammo.field.sound-anticipate')}
              />
            </div>
          </section>
        );
      })}
    </div>
  );
}
