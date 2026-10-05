import type { CustomSoundDto } from 'rimstudio-ipc-types';
import { Banner, Button } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { AssetStore } from '../../asset-store';
import { CLIP_FILTER, shotOf, shotWith, soundsWith } from '../../model/assets';
import { diagnosticsFor } from '../../model/draft';
import { AssetProblems } from './AssetProblems';
import { ClipRow } from './ClipRow';
import { useFieldEnv } from './fieldEnv';
import { NumberFieldRow } from './NumberFieldRow';
import { RangeRow } from './RangeRow';
import { TextFieldRow } from './TextFieldRow';

export interface CustomShotProps {
  assets: AssetStore;
}

/** The own clips of the shot sound, and the settings of the sound definition made from them. */
export function CustomShot({ assets }: CustomShotProps) {
  const env = useFieldEnv();
  const shot = shotOf(env.spec);
  const clips = shot?.clips ?? [];

  const write = (next: CustomSoundDto): void => env.setField('/sounds', soundsWith(env.spec, next));
  const add = async (): Promise<void> => {
    const picked = await assets.choose(CLIP_FILTER);
    if (picked && !clips.includes(picked)) write(shotWith(shot, 'clips', [...clips, picked]));
  };
  const removeAt = (index: number): void => {
    const next = clips.filter((_, i) => i !== index);
    // the last clip going away removes the whole custom sound, not just an empty list of clips
    if (next.length === 0 && Object.keys(shotWith(shot, 'clips', undefined)).length === 0) {
      env.setField('/sounds', soundsWith(env.spec, undefined));
    } else write(shotWith(shot, 'clips', next));
  };
  const error = assets.pickError.value;

  return (
    <div class="flex min-w-0 flex-col gap-3">
      <div data-field="/sounds/shot/clips" class="flex min-w-0 flex-col gap-2">
        <p class="text-small text-muted">{t('designer.sounds.clipsHelp')}</p>
        {error ? (
          <Banner tone="error" title={error.code}>
            {error.message}
          </Banner>
        ) : null}
        {clips.length > 0 ? (
          <ul aria-label={t('designer.sounds.clipsLabel')} class="flex flex-col gap-2">
            {clips.map((path, index) => (
              <ClipRow
                key={path}
                index={index}
                path={path}
                assets={assets}
                onRemove={() => removeAt(index)}
              />
            ))}
          </ul>
        ) : (
          <p class="text-small text-faint">{t('designer.sounds.noClips')}</p>
        )}
        <AssetProblems
          diagnostics={[
            ...diagnosticsFor(env.diagnostics, '/sounds/shot').filter(
              (d) => d.field === '/sounds/shot' || d.field === '/sounds/shot/clips',
            ),
          ]}
        />
        <div>
          <Button size="sm" icon="sound" onClick={() => void add()}>
            {t('designer.sounds.addClip')}
          </Button>
        </div>
      </div>
      <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
        <RangeRow
          pointer="/sounds/shot/volume"
          label={t('designer.sounds.volume')}
          help={t('designer.sounds.volumeHelp')}
          step={1}
          value={shot?.volume}
          onChange={(v) => write(shotWith(shot, 'volume', v))}
        />
        <RangeRow
          pointer="/sounds/shot/pitch"
          label={t('designer.sounds.pitch')}
          help={t('designer.sounds.pitchHelp')}
          step={0.01}
          value={shot?.pitch}
          onChange={(v) => write(shotWith(shot, 'pitch', v))}
        />
        <RangeRow
          pointer="/sounds/shot/distance"
          label={t('designer.sounds.distance')}
          help={t('designer.sounds.distanceHelp')}
          step={1}
          value={shot?.distance}
          onChange={(v) => write(shotWith(shot, 'distance', v))}
        />
        <NumberFieldRow
          def={{
            pointer: '/sounds/shot/maxSimultaneous',
            label: 'designer.sounds.maxSimultaneous',
            step: 1,
            plain: true,
          }}
        />
      </div>
      <TextFieldRow
        pointer="/sounds/shot/defName"
        label={t('designer.sounds.defName')}
        help={t('designer.sounds.defNameHelp')}
      />
    </div>
  );
}
