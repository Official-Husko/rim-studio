import type { AskItemDto, ConvertCandidateDto, CustomAmmoDto } from 'rimstudio-ipc-types';
import type { ComboboxOption } from 'rimstudio-ui';
import { AmmoSetField, type QuickPick } from '~/shared/ammo';
import { t } from '~/shared/i18n';
import type { ProjectRef } from '~/shared/project';
import { planConversion } from './api';
import { ammoBlock, setAnswer, setBlock, type AnswerScope } from './answerStore';
import { requestWithCustomAmmo } from './ammoRequest';
import { withOpenProject } from './session';

export interface PatchAmmoAskProps {
  candidate: ConvertCandidateDto;
  ask: AskItemDto;
  scope: AnswerScope;
  /** The ammo set answered for the weapon, or undefined. */
  value: string | undefined;
  /** The choices of the ask with the ranked ones first. */
  options: readonly ComboboxOption[];
  project: ProjectRef | undefined;
}

function picksOf(options: readonly ComboboxOption[]): QuickPick[] {
  return options
    .filter((o) => o.hint !== undefined)
    .slice(0, 5)
    .map((o) => ({ name: o.value, ...(o.hint ? { hint: o.hint } : {}) }));
}

/**
 * The question about the ammo set of a weapon to convert: the same field as on the Weapons page, with the
 * browser of every ammo set and the window for a custom caliber. A custom caliber writes its definition file
 * into the project with the conversion, so the window is offered when a project is open.
 */
export function PatchAmmoAsk({
  candidate,
  ask,
  scope,
  value,
  options,
  project,
}: PatchAmmoAskProps) {
  const custom = ammoBlock(candidate)?.customAmmo;
  return (
    <AmmoSetField
      label={ask.label}
      value={value}
      custom={custom}
      quickPicks={picksOf(options)}
      required
      reason={ask.reason}
      onSelect={(ammoSet) => {
        setBlock(candidate, scope, { customAmmo: undefined });
        setAnswer(candidate, scope, ask, ammoSet);
      }}
      onCustomChange={(next: CustomAmmoDto | undefined) => {
        setAnswer(candidate, scope, ask, undefined);
        setBlock(candidate, scope, { customAmmo: next });
      }}
      plan={
        project
          ? (next) =>
              withOpenProject(project, (id) =>
                planConversion(
                  id,
                  candidate.kind ?? 'ranged',
                  requestWithCustomAmmo(candidate, scope, next),
                ),
              )
          : undefined
      }
      canCreate={project !== undefined}
      createHint={project ? undefined : t('patches.ammo.needs-project')}
    />
  );
}
