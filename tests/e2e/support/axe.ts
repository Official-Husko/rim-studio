import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import type { Page } from '@playwright/test';

const require = createRequire(import.meta.url);
const source = readFileSync(require.resolve('axe-core/axe.min.js'), 'utf8');

export interface Violation {
  id: string;
  impact: string | null | undefined;
  help: string;
  nodes: { target: string[]; summary: string }[];
}

/** Runs axe-core on the current page and returns the violations. */
export async function axeViolations(page: Page, include?: string): Promise<Violation[]> {
  await page.evaluate(source);
  return page.evaluate(async (scope) => {
    const axe = (window as unknown as { axe: { run: (c: unknown, o: unknown) => Promise<{ violations: unknown[] }> } }).axe;
    const result = await axe.run(scope ? { include: [scope] } : document, {
      runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'best-practice'] },
    });
    return (result.violations as { id: string; impact: string; help: string; nodes: { target: string[]; failureSummary: string }[] }[]).map((v) => ({
      id: v.id,
      impact: v.impact,
      help: v.help,
      nodes: v.nodes.slice(0, 6).map((n) => ({ target: n.target, summary: n.failureSummary })),
    }));
  }, include);
}

/** Formats violations for a failure message. */
export function describe(violations: Violation[]): string {
  return violations
    .map((v) => `${v.id} (${v.impact}): ${v.help}\n${v.nodes.map((n) => `   ${n.target.join(' ')}`).join('\n')}`)
    .join('\n');
}
