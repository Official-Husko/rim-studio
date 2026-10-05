import type { DesignerCloneDiffResponse, JsonValue } from 'rimstudio-ipc-types';
import { Banner, Panel } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';
import { readoutLabel, readoutValue, signed, unitText } from '../../model/labels';

export interface ClonePanelProps {
  diff: DesignerCloneDiffResponse | undefined;
}

function show(value: JsonValue | undefined): string {
  if (value === undefined || value === null) return '-';
  if (typeof value === 'number') return formatNumber(value, 3);
  if (typeof value === 'string') return value;
  return JSON.stringify(value);
}

/** What differs from the source weapon, and what that does to the numbers. */
export function ClonePanel({ diff }: ClonePanelProps) {
  if (!diff) return null;
  const moved = diff.readouts.filter((r) => r.delta !== undefined && r.delta !== 0);
  return (
    <Panel title={t('designer.panel.clone', { source: diff.sourceLabel })} collapsible>
      <div class="flex flex-col gap-3">
        {diff.changes.length === 0 ? (
          <p class="text-small text-muted">{t('designer.clone.unchanged')}</p>
        ) : (
          <table class="w-full text-body" aria-label={t('designer.clone.changes')}>
            <thead class="text-left text-small text-muted">
              <tr>
                <th class="font-semibold">{t('designer.clone.field')}</th>
                <th class="text-right font-semibold">{t('designer.clone.old')}</th>
                <th class="text-right font-semibold">{t('designer.clone.new')}</th>
              </tr>
            </thead>
            <tbody>
              {diff.changes.map((c) => (
                <tr key={c.field} class="border-t border-line-subtle">
                  <td>{c.label}</td>
                  <td class="text-right font-mono text-mono tabular-nums">{show(c.old)}</td>
                  <td class="text-right font-mono text-mono tabular-nums text-accent">
                    {show(c.new)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {moved.length > 0 ? (
          <table class="w-full text-body" aria-label={t('designer.clone.effects')}>
            <thead class="text-left text-small text-muted">
              <tr>
                <th class="font-semibold">{t('designer.clone.readout')}</th>
                <th class="text-right font-semibold">{t('designer.clone.old')}</th>
                <th class="text-right font-semibold">{t('designer.clone.new')}</th>
                <th class="text-right font-semibold">{t('designer.clone.delta')}</th>
              </tr>
            </thead>
            <tbody>
              {moved.map((r) => (
                <tr key={r.key} class="border-t border-line-subtle">
                  <td>
                    {readoutLabel(r.key)} <span class="text-faint">{unitText(r.unit)}</span>
                  </td>
                  <td class="text-right font-mono text-mono tabular-nums">{readoutValue(r.old)}</td>
                  <td class="text-right font-mono text-mono tabular-nums">{readoutValue(r.new)}</td>
                  <td class="text-right font-mono text-mono tabular-nums">{signed(r.delta)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : null}
        {(diff.notes ?? []).map((note) => (
          <Banner key={note} tone="warning">
            {note}
          </Banner>
        ))}
      </div>
    </Panel>
  );
}
