import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { axeViolations, describe } from '../support/axe.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';
import { copyGewehrWithPatch, copyLoneWolf, snapshot } from '../support/mods.ts';
import { rememberProject } from '../support/session.ts';

test.describe.configure({ mode: 'serial' });

test.describe('project: the layout fixes on copies of the owner mods', () => {
  test('Gewehr 41: plan, apply with Fix all safe, see the tree improve, undo restores every byte', async ({
    page,
    env,
    shot,
  }) => {
    const mod = copyGewehrWithPatch(join(env.tmp, 'work-layout'));
    const before = snapshot(mod);
    expect(existsSync(join(mod, 'Patches', 'ce_patch.xml'))).toBe(true);
    expect(existsSync(join(mod, 'LoadFolders.xml'))).toBe(false);

    await rememberProject(page, mod);
    await gotoRoute(page, env, '/project');
    await page.getByRole('tab', { name: 'Files' }).click();
    await expect(page.getByRole('tree')).toBeVisible();
    await page.getByRole('tab', { name: /^Layout( [0-9]+)?$/ }).click();
    const issues = page.getByRole('list', { name: 'Layout issues' });
    await expect(issues).toContainText('Patches/ce_patch.xml');
    await expect(issues).toContainText('Defs/SoundDefs');
    await shot('layout-fix-findings');

    await page.getByRole('button', { name: 'Fix all safe' }).click();
    const dialog = page.getByRole('dialog', { name: 'Fix the layout' });
    const changes = dialog.getByRole('list', { name: 'Changes to apply' });
    await expect(changes).toBeVisible();
    await expect(changes).toContainText('Move file');
    await expect(changes).toContainText('Compat/CombatExtended/Patches/ce_patch.xml');
    await expect(changes).toContainText('LoadFolders.xml');
    // the LoadFolders.xml edit is shown as a diff before anything is written
    await expect(dialog.getByLabel('Edit of LoadFolders.xml')).toBeVisible();
    await expect(dialog.getByText('Safe').first()).toBeVisible();
    // planning writes nothing
    expect(snapshot(mod)).toEqual(before);
    await shot('layout-fix-review');
    expect(describe(await axeViolations(page)), 'axe fix review').toBe('');

    await dialog.getByRole('button', { name: /^Continue with [0-9]+ changes?$/ }).click();
    await expect(dialog.getByText(/undo journal/i).first()).toBeVisible();
    await shot('layout-fix-confirm');
    expect(describe(await axeViolations(page)), 'axe fix confirm').toBe('');
    // still nothing written
    expect(snapshot(mod)).toEqual(before);
    await dialog.getByRole('button', { name: 'Apply', exact: true }).click();
    await expect(dialog.getByText(/changes? carried out/)).toBeVisible({ timeout: 60_000 });
    await expect(dialog.getByText('The layout check finds nothing now.')).toBeVisible();
    await shot('layout-fix-result');
    expect(describe(await axeViolations(page)), 'axe fix result').toBe('');

    // on disk: the patch moved into the gated folder, LoadFolders.xml lists it, the folders exist
    expect(existsSync(join(mod, 'Patches', 'ce_patch.xml'))).toBe(false);
    const moved = join(mod, 'Compat', 'CombatExtended', 'Patches', 'ce_patch.xml');
    expect(existsSync(moved)).toBe(true);
    expect(readFileSync(moved, 'utf8')).toContain('CombatExtended');
    const loadFolders = readFileSync(join(mod, 'LoadFolders.xml'), 'utf8');
    expect(loadFolders).toContain('Compat/CombatExtended');
    expect(loadFolders).toContain('ceteam.combatextended');
    expect(existsSync(join(mod, 'Defs', 'SoundDefs'))).toBe(true);

    await dialog.getByRole('button', { name: 'Close' }).last().click();
    await expect(dialog).toBeHidden();
    // the check and the tree were read again
    await expect(page.getByText('The layout is in order')).toBeVisible();
    await page.getByRole('tab', { name: 'Files' }).click();
    await expect(page.getByRole('tree').getByRole('treeitem', { name: /^Compat/ }).first()).toBeVisible();
    await expect(page.getByRole('tree').getByRole('treeitem', { name: /^LoadFolders\.xml/ }).first()).toBeVisible();
    await shot('layout-fix-tree');

    // History lists the journal and undoes it: every file and folder is back as it was
    await page.getByRole('tab', { name: /^Layout( [0-9]+)?$/ }).click();
    await page.getByRole('button', { name: 'History' }).click();
    const history = page.getByRole('dialog', { name: /Fix history/ });
    await expect(history.getByRole('list', { name: 'Applied fixes' })).toContainText('Can be undone');
    await shot('layout-fix-history');
    await history.getByRole('button', { name: 'Undo', exact: true }).click();
    await expect(history.getByText('Undone').first()).toBeVisible({ timeout: 30_000 });
    await shot('layout-fix-undone');
    expect(snapshot(mod)).toEqual(before);
    await history.getByRole('button', { name: 'Close' }).last().click();
    await expect(page.getByRole('list', { name: 'Layout issues' })).toContainText('Patches/ce_patch.xml');
  });

  test('Lone Wolf: fix one finding, undo it from the result dialog', async ({ page, env, shot }) => {
    const mod = copyLoneWolf(join(env.tmp, 'work-layout'));
    const before = snapshot(mod);
    await rememberProject(page, mod);
    await gotoRoute(page, env, '/project');
    await page.getByRole('tab', { name: 'Files' }).click();
    await expect(page.getByRole('tree')).toBeVisible();
    await page.getByRole('tab', { name: /^Layout( [0-9]+)?$/ }).click();
    await expect(page.getByRole('list', { name: 'Layout issues' })).toContainText('Patches/CE_Patch.xml');
    await shot('layout-fix-lonewolf-findings');

    await page.getByRole('button', { name: 'Fix Patches/CE_Patch.xml' }).click();
    const dialog = page.getByRole('dialog', { name: 'Fix the layout' });
    const changes = dialog.getByRole('list', { name: 'Changes to apply' });
    // the mod keeps its code under Common, so the gated folder is made there
    await expect(changes).toContainText('Common/Compat/CombatExtended/Patches/CE_Patch.xml');
    await dialog.getByRole('button', { name: /^Continue with [0-9]+ changes?$/ }).click();
    await dialog.getByRole('button', { name: 'Apply', exact: true }).click();
    await expect(dialog.getByText(/changes? carried out/)).toBeVisible({ timeout: 60_000 });
    expect(existsSync(join(mod, 'Common', 'Compat', 'CombatExtended', 'Patches', 'CE_Patch.xml'))).toBe(true);
    expect(existsSync(join(mod, 'Patches', 'CE_Patch.xml'))).toBe(false);
    expect(existsSync(join(mod, 'LoadFolders.xml'))).toBe(true);
    await shot('layout-fix-lonewolf-result');

    await dialog.getByRole('button', { name: 'Undo this fix' }).click();
    await expect(dialog.getByText('Undone').first()).toBeVisible({ timeout: 30_000 });
    expect(snapshot(mod)).toEqual(before);
    await shot('layout-fix-lonewolf-undone');
  });

  test('Gewehr 41 at 1024 by 700: the fix dialog fits the window', async ({ page, env, shot }) => {
    const mod = copyGewehrWithPatch(join(env.tmp, 'work-layout-narrow'));
    await page.setViewportSize({ width: 1024, height: 700 });
    await rememberProject(page, mod);
    await gotoRoute(page, env, '/project');
    await page.getByRole('tab', { name: /^Layout( [0-9]+)?$/ }).click();
    await shot('w1024-layout-findings');
    await page.getByRole('button', { name: 'Fix all safe' }).click();
    const dialog = page.getByRole('dialog', { name: 'Fix the layout' });
    await expect(dialog.getByRole('list', { name: 'Changes to apply' })).toBeVisible();
    const box = await dialog.boundingBox();
    expect(box?.width ?? 0).toBeLessThanOrEqual(1024);
    expect(box?.height ?? 0).toBeLessThanOrEqual(700);
    await shot('w1024-layout-fix-review');
    expect(describe(await axeViolations(page)), 'axe fix review narrow').toBe('');
  });
});
