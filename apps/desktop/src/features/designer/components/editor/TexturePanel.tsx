import type { WritePlanDto } from 'rimstudio-ipc-types';
import { Banner, Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { AssetStore } from '../../asset-store';
import { hasOwnProjectile } from '../../model/assets';
import { useFieldEnv } from './fieldEnv';
import { TextureSlotView } from './TextureSlot';

export interface TexturePanelProps {
  assets: AssetStore;
  /** The plan of the open draft, for the target paths of the imports. */
  plan: WritePlanDto | undefined;
}

/** The art of the weapon: a PNG imported into the mod, for the weapon and for an own projectile. */
export function TexturePanel({ assets, plan }: TexturePanelProps) {
  const env = useFieldEnv();
  const ranged = env.spec.ranged !== undefined;
  const projectile =
    ranged && (hasOwnProjectile(env.spec) || env.spec.assets?.projectileTexture !== undefined);
  const error = assets.pickError.value;
  return (
    <Panel title={t('designer.panel.texture')} collapsible>
      <div class="flex flex-col gap-3">
        <p class="text-small text-muted">{t('designer.assets.neverCopied')}</p>
        {error ? (
          <Banner tone="error" title={error.code}>
            {error.message}
          </Banner>
        ) : null}
        <TextureSlotView slot="texture" assets={assets} plan={plan} />
        {projectile ? (
          <TextureSlotView slot="projectileTexture" assets={assets} plan={plan} />
        ) : ranged ? (
          <p class="text-small text-faint">{t('designer.assets.projectileNeedsOwn')}</p>
        ) : null}
      </div>
    </Panel>
  );
}
