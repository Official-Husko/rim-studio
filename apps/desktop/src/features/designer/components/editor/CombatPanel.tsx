import { Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { ACCURACY_FIELDS, RANGED_FIELDS } from '../../model/fields';
import { NumberFieldRow } from './NumberFieldRow';

export interface CombatPanelProps {
  kind: 'ranged' | 'melee';
}

/** The combat numbers of a ranged weapon; a melee weapon keeps its numbers in the tools. */
export function CombatPanel({ kind }: CombatPanelProps) {
  if (kind === 'melee') {
    return (
      <Panel title={t('designer.panel.combat')} collapsible>
        <p class="text-small text-muted">{t('designer.combat.melee')}</p>
      </Panel>
    );
  }
  return (
    <Panel title={t('designer.panel.combat')} collapsible>
      <div class="grid grid-cols-2 gap-3 xl:grid-cols-3">
        {RANGED_FIELDS.map((def) => (
          <NumberFieldRow key={def.pointer} def={def} />
        ))}
      </div>
      <h3 class="mt-4 mb-2 font-display text-label tracking-label text-muted uppercase">
        {t('designer.accuracy')}
      </h3>
      <div class="grid grid-cols-2 gap-3 xl:grid-cols-4">
        {ACCURACY_FIELDS.map((def) => (
          <NumberFieldRow key={def.pointer} def={def} />
        ))}
      </div>
    </Panel>
  );
}
