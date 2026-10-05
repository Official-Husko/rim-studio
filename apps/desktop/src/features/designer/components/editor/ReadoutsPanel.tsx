import type { ReadoutDto } from 'rimstudio-ipc-types';
import { Panel, Spinner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { readoutLabel, readoutValue, unitText } from '../../model/labels';

export interface ReadoutsPanelProps {
  readouts: readonly ReadoutDto[] | undefined;
  busy: boolean;
}

function Tile({ readout }: { readout: ReadoutDto }) {
  const unit = unitText(readout.unit);
  return (
    <div
      data-readout={readout.key}
      class="flex min-w-0 flex-col gap-1 border border-line-subtle bg-bg p-2"
    >
      <span class="text-small text-muted">{readoutLabel(readout.key)}</span>
      <span class="font-mono text-mono-display text-fg tabular-nums">
        {readoutValue(readout.value)}
        {unit && readout.value !== undefined ? (
          <span class="ml-1 text-mono-small text-faint">{unit}</span>
        ) : null}
      </span>
      {readout.steps && readout.steps.length > 0 ? (
        <details class="text-small text-muted">
          <summary class="cursor-pointer">{t('designer.readout.steps')}</summary>
          <ul class="mt-1 flex flex-col gap-0.5 font-mono text-mono-small">
            {readout.steps.map((step) => (
              <li key={step.label} class="flex justify-between gap-2">
                <span>{step.label}</span>
                <span class="tabular-nums">{readoutValue(step.value)}</span>
              </li>
            ))}
          </ul>
        </details>
      ) : null}
    </div>
  );
}

/** The exact numbers of the design: cycle time, DPS, strength and price. They update live. */
export function ReadoutsPanel({ readouts, busy }: ReadoutsPanelProps) {
  return (
    <Panel
      title={t('designer.panel.readouts')}
      actions={busy ? <Spinner size="sm" label={t('designer.readout.updating')} /> : null}
    >
      {readouts === undefined ? (
        <p class="text-small text-muted">{t('designer.readout.waiting')}</p>
      ) : (
        <div class="grid grid-cols-2 gap-2 lg:grid-cols-4">
          {readouts.map((readout) => (
            <Tile key={readout.key} readout={readout} />
          ))}
        </div>
      )}
    </Panel>
  );
}
