import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { readout } from '../support/designer.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';
import { rememberProject } from '../support/session.ts';

test.describe.configure({ mode: 'serial' });

const timings: Record<string, number> = {};

async function timed(name: string, run: () => Promise<void>): Promise<number> {
  const start = Date.now();
  await run();
  const ms = Date.now() - start;
  timings[name] = ms;
  process.stdout.write(`perf ${name}: ${ms} ms\n`);
  return ms;
}

async function openWeapons(page: Page, project: string, env: Parameters<typeof gotoRoute>[1]): Promise<void> {
  await rememberProject(page, project);
  await gotoRoute(page, env, '/weapons');
}

test.describe('performance sanity with the real install', () => {
  test('reference list, clone plan and edit feel instant', async ({ page, env }) => {
    const project = join(env.tmp, 'work', 'QA Test Mod');
    await openWeapons(page, project, env);
    const list = page.getByRole('list', { name: 'Reference weapons' });
    const first = await timed('reference list, first load', async () => {
      await expect(list.getByRole('listitem').first()).toBeVisible({ timeout: 45_000 });
    });
    expect(first).toBeLessThan(15_000);

    const melee = await timed('reference list, switch to melee', async () => {
      await page.getByRole('radio', { name: 'Melee' }).click();
      await expect(page.getByText(/of [0-9]+ weapons/)).not.toHaveText('20 of 20 weapons');
      await expect(list.getByRole('listitem').first()).toBeVisible();
    });
    expect(melee).toBeLessThan(2000);
    await page.getByRole('radio', { name: 'Ranged' }).click();

    const filter = await timed('reference list, filter by name', async () => {
      await page.getByPlaceholder('Search by name').fill('bolt');
      await expect(page.getByText('1 of 20 weapons')).toBeVisible();
    });
    expect(filter).toBeLessThan(1000);

    await list.getByRole('button', { name: 'Clone' }).first().click();
    const dialog = page.getByRole('dialog', { name: /^Clone / });
    await dialog.getByRole('textbox', { name: 'Def name' }).fill('Gun_QaPerf');
    const clone = await timed('clone: create the draft and show the plan', async () => {
      await dialog.getByRole('button', { name: 'Create' }).click();
      await expect(page.getByRole('button', { name: /^Preview Defs\// }).first()).toBeVisible();
      await expect(page.getByRole('region', { name: 'Live readouts' }).getByText('DPS', { exact: true })).toBeVisible();
    });
    expect(clone).toBeLessThan(3000);

    const before = await readout(page, 'DPS');
    const edit = await timed('edit the damage and see the readout change', async () => {
      await page.getByRole('spinbutton', { name: 'Damage', exact: true }).fill('40');
      await expect.poll(() => readout(page, 'DPS'), { timeout: 10_000 }).toBeGreaterThan(before);
    });
    expect(edit).toBeLessThan(1500);
  });

  test('a quiz step feels instant', async ({ page, env }) => {
    const project = join(env.tmp, 'work', 'QA Test Mod');
    await openWeapons(page, project, env);
    await page.getByRole('button', { name: 'New ranged' }).click();
    const d = page.getByRole('dialog');
    await d.getByRole('textbox', { name: 'Def name' }).fill('Gun_QaQuizPerf');
    await d.getByRole('button', { name: 'Create' }).click();
    await page.getByRole('radio', { name: 'Calibrate' }).click();
    const start = await timed('quiz: start', async () => {
      await page.getByRole('button', { name: 'Start the quiz' }).click();
      await expect(page.getByRole('dialog', { name: 'Estimate quiz' }).getByText(/Question 1 of about/)).toBeVisible();
    });
    expect(start).toBeLessThan(2000);
    const quiz = page.getByRole('dialog', { name: 'Estimate quiz' });
    const steps: number[] = [];
    for (let n = 2; n <= 4; n += 1) {
      steps.push(
        await timed(`quiz: step to question ${n}`, async () => {
          await quiz.getByRole('button', { name: 'Skip question' }).click();
          await expect(quiz.getByText(new RegExp(`Question ${n} of about`))).toBeVisible();
        }),
      );
    }
    expect(Math.max(...steps)).toBeLessThan(1000);
  });

  test.afterAll(() => {
    const dir = process.env.RIMSTUDIO_E2E_SHOTS;
    if (dir) writeFileSync(join(dir, 'perf.json'), `${JSON.stringify(timings, null, 2)}\n`);
  });
});
