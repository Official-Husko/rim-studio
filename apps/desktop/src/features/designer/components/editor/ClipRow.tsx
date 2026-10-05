import { useEffect } from 'preact/hooks';
import { IconButton } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { AssetStore } from '../../asset-store';
import { baseName } from '../../model/assets';
import { diagnosticsFor } from '../../model/draft';
import { AssetFacts } from './AssetFacts';
import { AssetProblems, mergeProblems } from './AssetProblems';
import { useFieldEnv } from './fieldEnv';

export interface ClipRowProps {
  /** Position of the clip in the custom sound. */
  index: number;
  path: string;
  assets: AssetStore;
  onRemove: () => void;
}

/** One clip file of the custom sound: its name, the facts the backend read and its problems. */
export function ClipRow({ index, path, assets, onRemove }: ClipRowProps) {
  const env = useFieldEnv();
  const state = assets.facts.value.get(path);
  useEffect(() => {
    void assets.load(path);
  }, [assets, path]);
  const problems = mergeProblems(
    diagnosticsFor(env.diagnostics, `/sounds/shot/clips/${index}`),
    state?.status === 'ready' ? (state.info.diagnostics ?? []) : [],
  );
  const name = baseName(path);
  return (
    <li
      data-field={`/sounds/shot/clips/${index}`}
      class="flex min-w-0 flex-col gap-2 border border-line p-2"
    >
      <div class="flex items-start justify-between gap-2">
        <span class="min-w-0">
          <span class="block truncate font-mono text-mono font-semibold text-fg">{name}</span>
          <span class="block break-all font-mono text-mono-small text-faint">{path}</span>
        </span>
        <IconButton
          icon="trash"
          label={t('designer.sounds.removeClip', { name })}
          onClick={onRemove}
        />
      </div>
      <AssetFacts path={path} state={state} />
      <AssetProblems diagnostics={problems} />
    </li>
  );
}
