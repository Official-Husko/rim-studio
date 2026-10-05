import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { chooseAmmoSet } from '../support/ammo.ts';
import { openWeaponsOn } from '../support/designer.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';

test.describe.configure({ mode: 'serial' });

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

test.describe('weapons: the optional Combat Extended patch', () => {
  test('switch on, answer the asks, apply and read the patch files', async ({ page, env, shot }) => {
    const project = join(env.tmp, 'work', 'QA Test Mod');
    await gotoRoute(page, env, '/weapons');
    await openWeaponsOn(page, project);
    await page.getByRole('button', { name: /bolt-action rifle copy/ }).first().click();
    await expect(page.getByRole('heading', { level: 2, name: /copy$/ }).first()).toBeVisible();

    // Vanilla by default: the switch starts off and no Combat Extended file is planned.
    const ce = page.getByRole('region', { name: 'Combat Extended', exact: true });
    const toggle = ce.getByRole('switch', { name: 'Add a Combat Extended patch (optional)' });
    await expect(toggle).not.toBeChecked();
    await toggle.click();
    await expect(toggle).toBeChecked();
    await expect(ce.getByRole('heading', { name: /answers? still needed/ })).toBeVisible({ timeout: 30_000 });
    await shot('ce-asks');

    // The caliber and the class tag are never chosen for the user.
    await chooseAmmoSet(page, ce, 'AmmoSet_338Lapua');
    const projectile = ce.getByRole('combobox', { name: /Which projectile of the ammo set/ });
    await expect(projectile).toBeEnabled();
    const projectiles = await projectile.locator('option:not([disabled])').allTextContents();
    expect(projectiles.length).toBeGreaterThan(0);
    await projectile.selectOption({ index: 1 });
    const tag = ce.getByRole('combobox', { name: /Which weapon class tag/ });
    const tagLabel = (await tag.locator('option').allTextContents()).find((t) => /CE_AI_SR/.test(t)) ?? '';
    await tag.selectOption({ label: tagLabel });
    await ce.getByRole('radio', { name: 'All derived' }).check();
    // Unreliable estimates are never written: the two numbers it cannot predict are typed.
    await expect(ce.getByRole('heading', { name: '2 answers still needed' })).toBeVisible({ timeout: 30_000 });
    await ce.getByRole('spinbutton', { name: 'CE magazine size' }).fill('5');
    await ce.getByRole('spinbutton', { name: 'CE shot spread' }).fill('0.2');
    for (const tool of ['stock', 'barrel']) {
      const field = ce.getByRole('spinbutton', { name: `Blunt penetration of the tool ${tool}` });
      if ((await field.count()) > 0 && (await field.inputValue()) === '') await field.fill('2');
    }
    await expect(ce.getByRole('heading', { name: /answers? still needed/ })).toBeHidden({ timeout: 30_000 });
    await shot('ce-answered');

    const output = page.getByRole('region', { name: 'Output', exact: true });
    await expect(output.getByRole('button', { name: /^Preview Compat\/CombatExtended\// })).toBeVisible({ timeout: 30_000 });
    await expect(output.getByRole('button', { name: /^Preview LoadFolders\.xml/ })).toBeVisible();
    await shot('ce-plan');
    await expect(page.getByText('Waiting for the plan to update.')).toBeHidden();
    await expect(page.getByRole('region', { name: 'Editor' }).getByText('Saved', { exact: true })).toBeVisible();
    await expect(page.getByRole('region', { name: 'Diagnostics' }).getByText('No errors')).toBeVisible();
    await expect(output.getByRole('button', { name: 'Apply to project' })).toBeEnabled();
    await output.getByRole('button', { name: 'Apply to project' }).click();
    const dialog = page.getByRole('dialog', { name: 'Apply to project' });
    await shot('ce-apply-review');
    await dialog.getByRole('button', { name: /^Write [0-9]+ files?/ }).click();
    await expect(dialog.getByText(/Wrote [0-9]+ files?/)).toBeVisible({ timeout: 60_000 });
    await shot('ce-apply-report');

    const files = walk(project);
    const patch = files.find((f) => f.startsWith(join('Compat', 'CombatExtended')) && f.endsWith('.xml'));
    expect(patch, `files: ${files.join(', ')}`).toBeDefined();
    expect(existsSync(join(project, 'LoadFolders.xml'))).toBe(true);
    const loadFolders = readFileSync(join(project, 'LoadFolders.xml'), 'utf8');
    expect(loadFolders).toContain('ceteam.combatextended');
    const patchXml = readFileSync(join(project, patch ?? ''), 'utf8');
    expect(patchXml).toContain('Gun_QaRifle');
    expect(patchXml).toContain('AmmoSet_338Lapua');
    // the vanilla definition is still a plain definition without Combat Extended content
    const vanilla = readFileSync(join(project, 'Defs', 'ThingDefs_Misc', 'Weapons', 'RangedIndustrial', 'Gun_QaRifle.xml'), 'utf8');
    expect(vanilla).not.toContain('CombatExtended');
  });
});
