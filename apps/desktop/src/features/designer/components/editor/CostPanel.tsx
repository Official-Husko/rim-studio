import type { CostEntryDto } from 'rimstudio-ipc-types';
import { Button, IconButton, Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { COMMON_FIELDS } from '../../model/fields';
import { getAt } from '../../model/pointer';
import { useFieldEnv } from './fieldEnv';
import { NumberFieldRow } from './NumberFieldRow';
import { RecipeFields } from './RecipeFields';
import { TextFieldRow } from './TextFieldRow';
import { diagnosticsFor } from '../../model/draft';
import { DiagnosticNotes } from './DiagnosticNotes';

export interface CostPanelProps {
  kind: 'ranged' | 'melee';
}

/** Mass, work, price, the ingredients and (for melee) the stuff the weapon is made from. */
export function CostPanel({ kind }: CostPanelProps) {
  const env = useFieldEnv();
  const cost = (getAt(env.spec, '/costList') as CostEntryDto[] | undefined) ?? [];
  const costDiagnostics = diagnosticsFor(env.diagnostics, '/costList');
  return (
    <Panel title={t('designer.panel.cost')} collapsible>
      <div class="flex flex-col gap-4">
        <div class="grid grid-cols-2 gap-3 xl:grid-cols-3">
          {COMMON_FIELDS.map((def) => (
            <NumberFieldRow key={def.pointer} def={def} />
          ))}
        </div>
        <div data-field="/costList" class="flex flex-col gap-2">
          <h3 class="font-display text-label tracking-label text-muted uppercase">
            {t('designer.cost.list')}
          </h3>
          {cost.map((entry, index) => (
            <div key={index} class="flex items-end gap-2">
              <div class="min-w-0 flex-1">
                <TextFieldRow
                  pointer={`/costList/${index}/defName`}
                  label={t('designer.cost.ingredient')}
                  keepEmpty
                />
              </div>
              <div class="w-32 shrink-0">
                <NumberFieldRow
                  def={{
                    pointer: `/costList/${index}/count`,
                    label: 'designer.cost.count',
                    step: 1,
                    plain: true,
                  }}
                />
              </div>
              <IconButton
                icon="trash"
                label={t('designer.cost.remove', { name: entry.defName })}
                variant="danger"
                onClick={() => env.setField(`/costList/${index}`, undefined)}
              />
            </div>
          ))}
          <div>
            <Button
              size="sm"
              icon="plus"
              onClick={() => env.setField(`/costList/${cost.length}`, { defName: '', count: 1 })}
            >
              {t('designer.cost.add')}
            </Button>
          </div>
          <DiagnosticNotes diagnostics={costDiagnostics} />
        </div>
        {kind === 'melee' ? (
          <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
            <TextFieldRow
              pointer="/stuff/categories"
              diagnosticPointer="/stuff"
              label={t('designer.field.stuffCategories')}
              list
            />
            <NumberFieldRow
              def={{ pointer: '/stuff/count', label: 'designer.field.stuffCount', step: 1 }}
            />
          </div>
        ) : null}
        <TextFieldRow pointer="/researchPrerequisite" label={t('designer.field.research')} />
        <RecipeFields />
      </div>
    </Panel>
  );
}
