import { Button } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { moveBy, newTool, replaceAt, type CeBlock, type CeBlockPatch } from './blockModel';
import { Group } from './Group';
import { StringListField } from './StringListField';
import { ToolPlanRow } from './ToolPlanRow';

export interface CeToolPlanFieldsProps {
  block: CeBlock;
  onChange: (patch: CeBlockPatch) => void;
}

/**
 * The explicit tool list of a conversion. Without it the tools of the vanilla weapon are carried over;
 * with it the list is written as given, so a deliberate muzzle tool or a capacity split is a choice here.
 */
export function CeToolPlanFields({ block, onChange }: CeToolPlanFieldsProps) {
  const plan = block.toolPlan ?? [];
  const keep = block.keepToolFields ?? [];
  return (
    <Group
      title={t('ceblock.tools.title')}
      summary={plan.length > 0 ? tn('ceblock.tools.count', plan.length) : t('ceblock.tools.none')}
    >
      <p class="m-0 text-small text-muted">{t('ceblock.tools.help')}</p>
      <div class="flex flex-col gap-2">
        {plan.map((tool, index) => (
          <ToolPlanRow
            key={index}
            tool={tool}
            index={index}
            count={plan.length}
            onChange={(next) => onChange({ toolPlan: replaceAt(plan, index, next) })}
            onRemove={() => onChange({ toolPlan: replaceAt(plan, index, undefined) })}
            onMove={(delta) => onChange({ toolPlan: moveBy(plan, index, delta) })}
          />
        ))}
        <div class="flex flex-wrap gap-2 self-start">
          <Button
            size="sm"
            variant="secondary"
            onClick={() => onChange({ toolPlan: [...plan, newTool()] })}
          >
            {t('ceblock.tools.add')}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            onClick={() => onChange({ toolPlan: [...plan, newTool('muzzle')] })}
          >
            {t('ceblock.tools.addMuzzle')}
          </Button>
        </div>
      </div>
      <StringListField
        label={t('ceblock.tools.keep')}
        help={t('ceblock.tools.keepHelp')}
        values={keep}
        onChange={(keepToolFields) => onChange({ keepToolFields })}
      />
    </Group>
  );
}
