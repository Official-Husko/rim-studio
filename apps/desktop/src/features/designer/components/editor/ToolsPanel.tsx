import type { ToolSpecDto } from 'rimstudio-ipc-types';
import { Button, IconButton, Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { toolFields } from '../../model/fields';
import { useFieldEnv } from './fieldEnv';
import { NumberFieldRow } from './NumberFieldRow';
import { TextFieldRow } from './TextFieldRow';
import { ChipListField } from './ChipListField';
import { ToolExtras } from './ToolExtras';
import { diagnosticsFor } from '../../model/draft';
import { DiagnosticNotes } from './DiagnosticNotes';

export interface ToolsPanelProps {
  kind: 'ranged' | 'melee';
}

/** The attacks of a melee weapon, or the bash attacks of a gun. One block per tool. */
export function ToolsPanel({ kind }: ToolsPanelProps) {
  const env = useFieldEnv();
  const tools: readonly ToolSpecDto[] = env.spec.tools ?? [];
  const diagnostics = diagnosticsFor(env.diagnostics, '/tools').filter((d) => d.field === '/tools');
  return (
    <Panel
      title={kind === 'melee' ? t('designer.panel.tools') : t('designer.panel.toolsGun')}
      collapsible
      defaultCollapsed={kind === 'ranged'}
    >
      <div data-field="/tools" class="flex flex-col gap-4">
        {tools.length === 0 ? (
          <p class="text-small text-muted">{t('designer.tools.none')}</p>
        ) : null}
        {tools.map((tool, index) => (
          <fieldset key={index} class="flex flex-col gap-3 border border-line-subtle p-3">
            <legend class="flex items-center gap-2 px-1 text-small font-semibold text-muted">
              {t('designer.tools.number', { n: index + 1 })}
              <IconButton
                icon="trash"
                label={t('designer.tools.remove', { name: tool.label || String(index + 1) })}
                variant="danger"
                onClick={() => env.setField(`/tools/${index}`, undefined)}
              />
            </legend>
            <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
              <TextFieldRow
                pointer={`/tools/${index}/label`}
                label={t('designer.field.toolLabel')}
                keepEmpty
              />
              <ChipListField
                pointer={`/tools/${index}/capacities`}
                label={t('designer.field.capacities')}
                help={t('designer.help.capacities')}
                keepEmpty
              />
              <TextFieldRow
                pointer={`/tools/${index}/linkedBodyPartsGroup`}
                label={t('designer.field.bodyPart')}
              />
            </div>
            <div class="grid grid-cols-2 gap-3">
              {toolFields(index).map((def) => (
                <NumberFieldRow key={def.pointer} def={def} />
              ))}
            </div>
            <ToolExtras index={index} />
          </fieldset>
        ))}
        <div>
          <Button
            size="sm"
            icon="plus"
            onClick={() => env.setField(`/tools/${tools.length}`, { label: '', capacities: [] })}
          >
            {t('designer.tools.add')}
          </Button>
        </div>
        <DiagnosticNotes diagnostics={diagnostics} />
      </div>
    </Panel>
  );
}
