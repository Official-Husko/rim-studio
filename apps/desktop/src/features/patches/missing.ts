import type { ConvertCandidateDto, DiagnosticDto } from 'rimstudio-ipc-types';
import type { MissingItem } from '~/shared/ce';
import { t } from '~/shared/i18n';
import { formatNumber } from '~/shared/format';
import { NEEDS_ANSWER } from './model';

/** The field pointer an open question carries in a plan. */
function fieldOf(d: DiagnosticDto): string {
  return d.field ?? d.args?.field ?? '';
}

/** Why the generator needs an answer, in plain words (the ask's own reason first). */
function whyOf(candidate: ConvertCandidateDto, field: string): string | undefined {
  const ask = candidate.asks.find((a) => a.field === field);
  if (ask?.reason) return ask.reason;
  if (field === '/ce/ammoSet') return t('patches.missing.why.ammoSet');
  if (field === '/ce/weaponTagClass') return t('patches.missing.why.weaponTagClass');
  if (ask?.kind === 'number' && ask.suggestion !== undefined) {
    return t('patches.missing.estimate', { value: formatNumber(ask.suggestion, 3) });
  }
  return undefined;
}

/**
 * What a weapon is still missing before its patch can be generated: the questions the plan reports
 * as open and, before a plan exists, every question of the scan. Order follows the scan.
 */
export function missingOf(
  candidate: ConvertCandidateDto,
  diagnostics: readonly DiagnosticDto[] | undefined,
): MissingItem[] {
  if (diagnostics === undefined) {
    return candidate.asks.map((ask) => ({
      key: ask.field,
      label: ask.label,
      why: whyOf(candidate, ask.field),
    }));
  }
  const open = new Map<string, DiagnosticDto>();
  for (const d of diagnostics) {
    if (d.code !== NEEDS_ANSWER) continue;
    const field = fieldOf(d);
    if (!open.has(field || d.message)) open.set(field || d.message, d);
  }
  const order = new Map(candidate.asks.map((a, i) => [a.field, i]));
  return [...open.entries()]
    .sort(([a], [b]) => (order.get(a) ?? 1e6) - (order.get(b) ?? 1e6))
    .map(([key, d]) => {
      const ask = candidate.asks.find((a) => a.field === key);
      return { key, label: ask?.label ?? d.message, why: whyOf(candidate, key) };
    });
}

/** Moves the focus to the control of one question once the questions tab is on screen. */
export function focusAsk(field: string): void {
  const run = (): void => {
    const root = Array.from(document.querySelectorAll<HTMLElement>('[data-ask]')).find(
      (e) => e.getAttribute('data-ask') === field,
    );
    if (!root) return;
    root.scrollIntoView?.({ block: 'center' });
    root
      .querySelector<HTMLElement>(
        'input, select, textarea, [role="combobox"], [role="radio"], button',
      )
      ?.focus();
  };
  setTimeout(run, 60);
}
