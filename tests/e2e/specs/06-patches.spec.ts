import { existsSync, mkdirSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { chooseAmmoSet } from '../support/ammo.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';
import { copyGewehr } from '../support/mods.ts';
import { pickFolderPath } from '../support/ui.ts';

/** Every file under a folder, relative to it. */
function walk(root: string, dir = ''): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(join(root, dir), { withFileTypes: true })) {
    const rel = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...walk(root, rel));
    else out.push(rel);
  }
  return out;
}

test.describe('patches: convert a copy of a real mod', () => {
  test('scan, answer the questions, plan, apply and lint', async ({ page, env, shot }) => {
    const work = join(env.tmp, 'work');
    mkdirSync(work, { recursive: true });
    const mod = copyGewehr(work);
    expect(existsSync(join(mod, 'Patches'))).toBe(false);

    await gotoRoute(page, env, '/patches');
    await page.getByRole('button', { name: 'Choose a mod folder' }).click();
    await pickFolderPath(page, mod);
    const table = page.getByRole('grid', { name: 'Weapons of the mod' });
    await expect(table.getByRole('row')).toHaveCount(6, { timeout: 60_000 });
    await expect(page.getByText('5 not converted')).toBeVisible();
    await shot('patches-scan');

    // Answer the questions of the first weapon.
    const form = page.getByRole('tabpanel', { name: /^Questions/ });
    await expect(form.getByRole('button', { name: 'Browse all ammo' })).toBeVisible();
    await chooseAmmoSet(page, form, 'AmmoSet_303British');
    await form.getByRole('combobox', { name: /Which weapon class tag/ }).click();
    await page.getByRole('listbox').getByRole('option').first().click();
    await form.getByRole('radiogroup', { name: /one hand/ }).getByRole('radio', { name: 'No' }).click();
    await form.getByRole('radiogroup', { name: /belt fed/ }).getByRole('radio', { name: 'No' }).click();
    // Unreliable estimates are not written until the user takes them.
    const uses = form.getByRole('button', { name: /^Use [0-9.]+$/ });
    const count = await uses.count();
    expect(count).toBeGreaterThan(0);
    // taking an estimate answers the question and removes its hint
    for (let left = count; left > 0; left -= 1) {
      await uses.first().click({ timeout: 5000 });
      await expect(uses).toHaveCount(left - 1);
    }
    await expect(form.getByText(/^9 of 9 answered/)).toBeVisible();
    await shot('patches-answered');

    await page.getByRole('tab', { name: 'Plan' }).click();
    await expect(page.getByText(/Compat\/CombatExtended|Patches\//).first()).toBeVisible({ timeout: 30_000 });
    await shot('patches-plan');

    await page.getByRole('checkbox', { name: 'Convert Gewehr 41 (m)', exact: true }).check();
    const convert = page.getByRole('button', { name: 'Convert 1 weapon' });
    await expect(convert).toBeEnabled();
    await convert.click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await shot('patches-apply-review');
    await dialog.getByRole('button', { name: /^(Write|Apply|Convert)/ }).last().click();
    await expect(dialog.getByText(/written|Wrote|Applied|Done/i).first()).toBeVisible({ timeout: 90_000 });
    await shot('patches-apply-report');
    await dialog.getByRole('button', { name: /Close|Done/ }).first().click();

    const files = walk(mod);
    const patch = files.find((f) => f.startsWith(join('Compat', 'CombatExtended')) && f.endsWith('.xml'));
    expect(patch, `files: ${files.join(', ')}`).toBeDefined();
    expect(readFileSync(join(mod, patch ?? ''), 'utf8')).toContain('OH_G41m');
    expect(existsSync(join(mod, 'LoadFolders.xml'))).toBe(true);
    // the vanilla definitions are untouched by a conversion
    expect(readFileSync(join(mod, 'Defs', 'ThingDefs_Misc', 'Weapons', 'RangedIndustrial.xml'), 'utf8')).not.toContain('CombatExtended');

    // After the write the weapon reads as converted and the lint view runs on it.
    await page.getByRole('button', { name: 'Scan again' }).click();
    await expect(page.getByText('1 already CE')).toBeVisible({ timeout: 60_000 });
    await page.getByRole('radio', { name: 'Lint' }).click();
    // the lint view reads every patch file of the mod and names each one
    await expect(page.getByText('Files checked: 1.')).toBeVisible({ timeout: 60_000 });
    await expect(page.getByRole('heading', { name: /gewehr41_Weapons_Ranged\.xml$/ })).toBeVisible();
    await shot('patches-lint');
  });
});
