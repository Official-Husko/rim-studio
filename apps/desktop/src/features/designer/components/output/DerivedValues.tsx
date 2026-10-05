import type { DiagnosticDto } from 'rimstudio-ipc-types';
import { Panel } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';

export interface DerivedValuesProps {
  derived: readonly DiagnosticDto[];
}

/** The numbers the plan took from a suggestion instead of from the user, with where each came from. */
export function DerivedValues({ derived }: DerivedValuesProps) {
  if (derived.length === 0) return null;
  return (
    <Panel
      title={t('designer.output.derived')}
      collapsible
      defaultCollapsed
      actions={
        <span class="font-mono text-mono-small text-muted">
          {tn('designer.output.derivedCount', derived.length)}
        </span>
      }
    >
      <ul class="flex flex-col gap-2">
        {derived.map((d, i) => (
          <li key={`${d.field ?? ''}-${i}`} class="flex flex-col gap-0.5 text-small">
            <span class="font-mono text-mono-small text-fg">
              {d.field ?? ''} = {d.args?.value ?? ''}
            </span>
            <span class="text-muted">{d.args?.how ?? d.message}</span>
          </li>
        ))}
      </ul>
    </Panel>
  );
}
