import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test as base, type Page } from '@playwright/test';
import { readEnv, type E2eEnv } from './env.ts';

/** Console messages of the page that count as defects. */
export interface Problems {
  errors: string[];
}

interface Fixtures {
  env: E2eEnv;
  problems: Problems;
  shot: (name: string) => Promise<void>;
}

/** Messages that are expected and are not defects (none so far). */
const ALLOWED: RegExp[] = [];

function watch(page: Page, problems: Problems): void {
  page.on('console', (message) => {
    if (message.type() !== 'error') return;
    const text = message.text();
    if (ALLOWED.some((re) => re.test(text))) return;
    problems.errors.push(`console.error: ${text}`);
  });
  page.on('pageerror', (error) => problems.errors.push(`pageerror: ${error.message}`));
}

export const test = base.extend<Fixtures>({
  // eslint-disable-next-line no-empty-pattern
  env: async ({}, use) => {
    await use(readEnv());
  },
  // every test checks the console: an error message or an uncaught exception fails it
  problems: [
    async ({ page }, use) => {
      const problems: Problems = { errors: [] };
      watch(page, problems);
      await use(problems);
      expect(problems.errors, 'console errors and page errors').toEqual([]);
    },
    { auto: true },
  ],
  shot: async ({ page, env, problems }, use) => {
    void problems;
    const dir = process.env.RIMSTUDIO_E2E_SHOTS ?? env.shots;
    mkdirSync(dir, { recursive: true });
    await use(async (name: string) => {
      await page.screenshot({ path: join(dir, `${name}.png`) });
    });
  },
});

export { expect };

/** Opens a hash route and waits for the shell. */
export async function gotoRoute(page: Page, env: E2eEnv, hash: string): Promise<void> {
  await page.goto(`${env.baseUrl}/#${hash}`);
  await page.getByRole('navigation').first().waitFor();
}
