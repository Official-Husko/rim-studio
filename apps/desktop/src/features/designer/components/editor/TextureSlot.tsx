import { useEffect } from 'preact/hooks';
import type { WritePlanDto } from 'rimstudio-ipc-types';
import { Button } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { AssetStore } from '../../asset-store';
import { diagnosticsFor } from '../../model/draft';
import { assetsWith, TEXTURE_FILTER, textureOrigin, type TextureSlot } from '../../model/assets';
import { AssetFacts } from './AssetFacts';
import { AssetProblems, mergeProblems } from './AssetProblems';
import { useFieldEnv } from './fieldEnv';

export interface TextureSlotProps {
  slot: TextureSlot;
  assets: AssetStore;
  plan: WritePlanDto | undefined;
}

const POINTER: Record<TextureSlot, string> = {
  texture: '/assets/texture',
  projectileTexture: '/assets/projectileTexture',
};

/** One picture of the weapon: where it comes from, the file imported for it and what to do next. */
export function TextureSlotView({ slot, assets, plan }: TextureSlotProps) {
  const env = useFieldEnv();
  const pointer = POINTER[slot];
  const path = env.spec.assets?.[slot];
  const state = path === undefined ? undefined : assets.facts.value.get(path);
  const origin = textureOrigin(env.spec, slot, plan);
  const title =
    slot === 'texture'
      ? t('designer.assets.weaponTexture')
      : t('designer.assets.projectileTexture');

  useEffect(() => {
    if (path !== undefined) void assets.load(path);
  }, [assets, path]);

  const write = (next: string | undefined): void =>
    env.setField('/assets', assetsWith(env.spec, slot, next));
  const choose = async (): Promise<void> => {
    const picked = await assets.choose(TEXTURE_FILTER);
    if (picked) write(picked);
  };

  const problems = mergeProblems(
    diagnosticsFor(env.diagnostics, pointer),
    state?.status === 'ready' ? (state.info.diagnostics ?? []) : [],
  );
  const originText =
    origin.kind === 'imported'
      ? origin.target
        ? t('designer.assets.origin.imported', { target: origin.target })
        : t('designer.assets.origin.importedPending')
      : origin.kind === 'shared'
        ? t('designer.assets.origin.shared', { texPath: origin.texPath })
        : t('designer.assets.origin.reserved');

  return (
    <div data-field={pointer} class="flex min-w-0 flex-col gap-2 border border-line p-3">
      <h3 class="text-body font-semibold text-fg">{title}</h3>
      <p class="break-words text-small text-muted">{originText}</p>
      {path !== undefined ? (
        <>
          <p class="break-all font-mono text-mono-small text-faint">{path}</p>
          <AssetFacts path={path} state={state} />
        </>
      ) : null}
      <AssetProblems diagnostics={problems} />
      <div class="flex flex-wrap gap-2">
        <Button size="sm" icon="image" onClick={() => void choose()}>
          {path === undefined ? t('designer.assets.importPng') : t('designer.assets.replacePng')}
        </Button>
        {path !== undefined ? (
          <Button size="sm" variant="ghost" icon="trash" onClick={() => write(undefined)}>
            {t('designer.assets.removeImport')}
          </Button>
        ) : null}
      </div>
    </div>
  );
}
