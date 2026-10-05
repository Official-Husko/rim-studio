import type { CeChoiceDto, DesignSpecDto } from 'rimstudio-ipc-types';
import { AmmoSetField, type QuickPick } from '~/shared/ammo';
import { t, tn } from '~/shared/i18n';
import type { OutputStore } from '../../../output-store';

export interface AmmoSetAskProps {
  store: OutputStore;
  spec: DesignSpecDto;
  /** The question of the backend for the ammo set, with the ranked candidates. */
  choice: CeChoiceDto;
}

function picksOf(choice: CeChoiceDto): QuickPick[] {
  return choice.candidates.slice(0, 5).map((c) => ({
    name: c.name,
    ...(c.usedBy > 0 ? { hint: tn('designer.output.ce.usedBy', c.usedBy) } : {}),
  }));
}

/**
 * The ammo set of the weapon on the Weapons page: the chosen set or the custom ammunition, the ranked
 * suggestions, the browser of every ammo set and the window that makes a custom caliber. The window exists
 * here only because this component is shown with the Combat Extended switch on.
 */
export function AmmoSetAsk({ store, spec, choice }: AmmoSetAskProps) {
  const custom = spec.ce?.customAmmo;
  const open = store.hasProject();
  return (
    <div data-ce-field={choice.field}>
      <AmmoSetField
        label={choice.label}
        value={spec.ce?.ammoSet}
        custom={custom}
        quickPicks={picksOf(choice)}
        required={choice.required}
        reason={choice.reason}
        draft={store.currentDraft}
        onSelect={(ammoSet) =>
          store.patchBlock({ ammoSet, defaultProjectile: undefined, customAmmo: undefined })
        }
        onCustomChange={(next) =>
          store.patchBlock({ customAmmo: next, ammoSet: undefined, defaultProjectile: undefined })
        }
        plan={open ? store.planWithAmmo : undefined}
        canCreate
        createHint={open ? undefined : t('designer.output.ce.ammo.needs-project')}
      />
    </div>
  );
}
