import { useEffect, useState } from 'preact/hooks';
import type { ConvertCandidateDto } from 'rimstudio-ipc-types';
import { Banner, SegmentedControl } from 'rimstudio-ui';
import { CeExtrasEditor, type CeBlock } from '~/shared/ce';
import { t, tn } from '~/shared/i18n';
import {
  effectiveBlock,
  familyAnswers,
  ownAnswers,
  setBlock,
  type AnswerScope,
} from './answerStore';
import { familyOf } from './model';
import { loadOptions, optionSuggestions } from './optionsStore';

export interface OptionsFormProps {
  candidate: ConvertCandidateDto;
  /** How many weapons of the scan share the family key of this one (itself included). */
  familySize: number;
}

/**
 * The optional Combat Extended members of one weapon's conversion: the bow choice, ammo extras, companion
 * tags, the explicit tool list, platform and under barrel parameters and raw elements. Nothing here is
 * written unless it is set, and the same members can be set for a whole family of weapons.
 */
export function OptionsForm({ candidate, familySize }: OptionsFormProps) {
  const [scope, setScope] = useState<AnswerScope>('weapon');
  const own = ownAnswers.value;
  const family = familyAnswers.value;
  const shareable = familyOf(candidate) !== '' && familySize > 1;
  const activeScope: AnswerScope = shareable ? scope : 'weapon';
  const tagClass = own[candidate.defName]?.weaponTagClass;
  const familyClass = family[familyOf(candidate)]?.weaponTagClass;
  const block = {
    oneHanded: false,
    beltFed: false,
    ...effectiveBlock(candidate, own, family),
  } as CeBlock;
  const suggestions = optionSuggestions.value[candidate.defName] ?? [];
  const sets = candidate.asks.find((ask) => ask.field === '/ce/ammoSet')?.options ?? [];

  useEffect(() => {
    void loadOptions(candidate, { numbers: {}, weaponTagClass: tagClass ?? familyClass });
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [candidate.defName, tagClass, familyClass]);

  return (
    <div class="flex flex-col gap-4">
      <p class="m-0 text-small text-muted">{t('patches.options.help')}</p>
      {shareable ? (
        <div class="self-start">
          <SegmentedControl
            label={t('patches.scope.label')}
            value={scope}
            onValueChange={(v) => setScope(v === 'family' ? 'family' : 'weapon')}
            options={[
              { value: 'weapon', label: t('patches.scope.weapon') },
              { value: 'family', label: tn('patches.scope.family', familySize) },
            ]}
          />
        </div>
      ) : null}
      {shareable && activeScope === 'family' ? (
        <Banner tone="info">
          {t('patches.options.familyHelp', { family: familyOf(candidate) })}
        </Banner>
      ) : null}
      <CeExtrasEditor
        block={block}
        onChange={(patch) => setBlock(candidate, activeScope, patch)}
        options={suggestions}
        ammoSets={sets.map((name) => ({ value: name, label: name }))}
      />
    </div>
  );
}
