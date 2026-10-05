import type { CeUnderBarrelFireModesDto } from 'rimstudio-ipc-types';
import { FormField, NumberField, Switch, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { TriState } from './TriState';

export interface UnderBarrelFireModesProps {
  modes: CeUnderBarrelFireModesDto;
  onChange: (modes: CeUnderBarrelFireModesDto) => void;
}

/** The fire mode choices of the under barrel unit. */
export function UnderBarrelFireModes({ modes, onChange }: UnderBarrelFireModesProps) {
  const set = (patch: Partial<CeUnderBarrelFireModesDto>): void => {
    const next = { ...modes, ...patch };
    if (next.aiUseBurstMode === undefined) delete next.aiUseBurstMode;
    if (next.aiAimMode === undefined || next.aiAimMode === '') delete next.aiAimMode;
    if (next.aimedBurstShotCount === undefined) delete next.aimedBurstShotCount;
    onChange(next);
  };
  return (
    <div class="flex flex-col gap-3" role="group" aria-label={t('ceblock.ub.modes')}>
      <span class="text-small font-medium">{t('ceblock.ub.modes')}</span>
      <FormField label={t('ceblock.ub.aiBurst')}>
        <TriState
          label={t('ceblock.ub.aiBurst')}
          value={modes.aiUseBurstMode}
          onChange={(aiUseBurstMode) => set({ aiUseBurstMode })}
        />
      </FormField>
      <div class="grid grid-cols-2 gap-3">
        <FormField label={t('ceblock.ub.aiAim')} help={t('ceblock.ub.aiAimHelp')}>
          <TextField
            aria-label={t('ceblock.ub.aiAim')}
            value={modes.aiAimMode ?? ''}
            onValueChange={(aiAimMode) => set({ aiAimMode })}
          />
        </FormField>
        <FormField label={t('ceblock.ub.aimedBurst')}>
          <div class="w-32">
            <NumberField
              aria-label={t('ceblock.ub.aimedBurst')}
              value={modes.aimedBurstShotCount}
              step={1}
              min={0}
              onValueChange={(n) =>
                set({ aimedBurstShotCount: n === undefined ? undefined : Math.round(n) })
              }
            />
          </div>
        </FormField>
      </div>
      <Switch
        checked={modes.noSingleShot}
        onCheckedChange={(noSingleShot) => set({ noSingleShot })}
      >
        {t('ceblock.ub.noSingle')}
      </Switch>
    </div>
  );
}
