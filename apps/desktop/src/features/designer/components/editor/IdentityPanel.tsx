import type { TechLevelDto } from 'rimstudio-ipc-types';
import { FormField, Panel, Select, type SelectOption } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { diagnosticsFor } from '../../model/draft';
import { getAt } from '../../model/pointer';
import { DiagnosticNotes } from './DiagnosticNotes';
import { useFieldEnv } from './fieldEnv';
import { TextFieldRow } from './TextFieldRow';

/** The tiers of the contract, in order, with their display names. */
function tierOptions(): SelectOption[] {
  const tiers: Array<[TechLevelDto, string]> = [
    ['neolithic', t('designer.tier.neolithic')],
    ['medieval', t('designer.tier.medieval')],
    ['industrial', t('designer.tier.industrial')],
    ['spacer', t('designer.tier.spacer')],
    ['ultra', t('designer.tier.ultra')],
    ['archotech', t('designer.tier.archotech')],
  ];
  return tiers.map(([value, label]) => ({ value, label }));
}

function tierName(tier: TechLevelDto): string {
  return tierOptions().find((o) => o.value === tier)?.label ?? tier;
}

/** Names, tier, role and parent base: what the item is. */
export function IdentityPanel() {
  const env = useFieldEnv();
  const tier = getAt(env.spec, '/techLevel') as string | undefined;
  const role = getAt(env.spec, '/role') as string | undefined;
  const roleOptions: SelectOption[] = [...new Set([...env.roles, ...(role ? [role] : [])])].map(
    (r) => ({ value: r, label: r }),
  );
  const inheritedTier = env.spec.parent?.inheritedTechLevel;
  const tierDiagnostics = diagnosticsFor(env.diagnostics, '/techLevel');
  const roleDiagnostics = diagnosticsFor(env.diagnostics, '/role');
  const firstError = (list: typeof tierDiagnostics): string | undefined =>
    list.find((d) => d.severity === 'error')?.message;
  return (
    <Panel title={t('designer.panel.identity')} collapsible>
      <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
        <TextFieldRow pointer="/identity/defName" label={t('designer.field.defName')} keepEmpty />
        <TextFieldRow pointer="/identity/label" label={t('designer.field.label')} keepEmpty />
        <div class="md:col-span-2">
          <TextFieldRow
            pointer="/identity/description"
            label={t('designer.field.description')}
            multiline
            keepEmpty
          />
        </div>
        <div data-field="/techLevel" class="flex flex-col gap-1">
          <FormField label={t('designer.field.tier')} error={firstError(tierDiagnostics)} required>
            <Select
              value={tier}
              options={tierOptions()}
              placeholder={t('designer.pick')}
              onValueChange={(value) => env.setField('/techLevel', value)}
            />
          </FormField>
          {tier === undefined && inheritedTier ? (
            <p class="text-small text-faint">
              {t('designer.identity.tierInherited', { tier: tierName(inheritedTier) })}
            </p>
          ) : null}
          <DiagnosticNotes diagnostics={tierDiagnostics} />
        </div>
        <div data-field="/role" class="flex flex-col gap-1">
          <FormField label={t('designer.field.role')} error={firstError(roleDiagnostics)} required>
            <Select
              value={role}
              options={roleOptions}
              placeholder={t('designer.pick')}
              onValueChange={(value) => env.setField('/role', value)}
            />
          </FormField>
          <DiagnosticNotes diagnostics={roleDiagnostics} />
        </div>
        <div class="md:col-span-2">
          <TextFieldRow
            pointer="/parent/defName"
            diagnosticPointer="/parent"
            label={t('designer.field.parent')}
            help={t('designer.help.parent')}
          />
        </div>
      </div>
    </Panel>
  );
}
