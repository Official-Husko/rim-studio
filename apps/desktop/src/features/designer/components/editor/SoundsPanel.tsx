import { useState } from 'preact/hooks';
import type { WritePlanDto } from 'rimstudio-ipc-types';
import { Panel, SegmentedControl } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { AssetStore } from '../../asset-store';
import { shotOf, soundsWith } from '../../model/assets';
import { diagnosticsFor } from '../../model/draft';
import { AssetProblems } from './AssetProblems';
import { CustomShot } from './CustomShot';
import { useFieldEnv } from './fieldEnv';
import { SoundNameRow } from './SoundNameRow';
import { SoundPlanSummary } from './SoundPlanSummary';

export interface SoundsPanelProps {
  assets: AssetStore;
  plan: WritePlanDto | undefined;
}

type Which = 'shot' | 'tail' | 'equip';
type Mode = 'game' | 'own';

/**
 * The sounds of the weapon: the shot (a sound of the game, or clips of your own), the shot tail and
 * the sound of picking it up. A custom sound is copied into the mod with its own sound definition.
 */
export function SoundsPanel({ assets, plan }: SoundsPanelProps) {
  const env = useFieldEnv();
  const ranged = env.spec.ranged !== undefined;
  const shot = shotOf(env.spec);
  const [wantsOwn, setWantsOwn] = useState(shot !== undefined);
  const [open, setOpen] = useState<Which | undefined>(undefined);
  const mode: Mode = shot !== undefined || wantsOwn ? 'own' : 'game';

  const openFor = (which: Which) => ({
    open: open === which,
    onOpenChange: (next: boolean) => setOpen(next ? which : undefined),
  });
  const choose = (next: string): void => {
    if (next === 'own') setWantsOwn(true);
    else {
      setWantsOwn(false);
      if (shot !== undefined) env.setField('/sounds', soundsWith(env.spec, undefined));
    }
  };

  return (
    <Panel title={t('designer.panel.sounds')} collapsible>
      <div class="flex flex-col gap-4">
        <p class="text-small text-muted">{t('designer.sounds.help')}</p>
        {ranged ? (
          <div data-field="/sounds/shot" class="flex min-w-0 flex-col gap-3">
            <h3 class="text-body font-semibold text-fg">{t('designer.sounds.shot')}</h3>
            <SegmentedControl
              label={t('designer.sounds.shotSource')}
              value={mode}
              onValueChange={choose}
              options={[
                { value: 'game', label: t('designer.sounds.fromGame') },
                { value: 'own', label: t('designer.sounds.ownClips') },
              ]}
            />
            {mode === 'game' ? (
              <SoundNameRow
                pointer="/ranged/soundCast"
                label={t('designer.field.soundCast')}
                assets={assets}
                {...openFor('shot')}
              />
            ) : (
              <>
                <AssetProblems
                  diagnostics={diagnosticsFor(env.diagnostics, '/ranged/soundCast').filter(
                    (d) => d.code === 'design.sound-cast-replaced',
                  )}
                />
                <CustomShot assets={assets} />
                <SoundPlanSummary plan={plan} typedName={shot?.defName} />
              </>
            )}
            <SoundNameRow
              pointer="/ranged/soundCastTail"
              label={t('designer.field.soundTail')}
              assets={assets}
              {...openFor('tail')}
            />
          </div>
        ) : null}
        <SoundNameRow
          pointer="/soundInteract"
          label={t('designer.field.soundInteract')}
          help={t('designer.sounds.equipHelp')}
          assets={assets}
          {...openFor('equip')}
        />
      </div>
    </Panel>
  );
}
