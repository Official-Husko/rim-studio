import { cpSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { chooseAmmoSet } from '../support/ammo.ts';
import { axeViolations, describe } from '../support/axe.ts';
import { cloneReference, openWeaponsOn } from '../support/designer.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';
import { rememberProject } from '../support/session.ts';

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

/** A new empty mod with only an About file. */
function emptyMod(root: string, name: string, packageId: string): string {
  const work = join(root, name);
  mkdirSync(join(work, 'About'), { recursive: true });
  writeFileSync(
    join(work, 'About', 'About.xml'),
    '<?xml version="1.0" encoding="utf-8"?>\n' +
      `<ModMetaData><name>${name}</name><author>qa</author><packageId>${packageId}</packageId>` +
      '<supportedVersions><li>1.6</li></supportedVersions><description>qa</description></ModMetaData>\n',
  );
  return work;
}

test.describe('weapons: Combat Extended for bows and weapon platforms', () => {
  test('clone a vanilla bow and look at the Combat Extended block', async ({ page, env, shot }) => {
    const work = emptyMod(join(env.tmp, 'work-bows'), 'Bows Mod', 'qa.bows');
    await gotoRoute(page, env, '/weapons');
    await openWeaponsOn(page, work);
    await cloneReference(page, 'short bow', 'Bow_QaShort');
    const ce = page.getByRole('region', { name: 'Combat Extended', exact: true });
    const toggle = ce.getByRole('switch', { name: 'Add a Combat Extended patch (optional)' });
    await expect(toggle).not.toBeChecked();
    await toggle.click();
    await expect(ce.getByRole('heading', { name: /answers? still needed/ })).toBeVisible({ timeout: 30_000 });
    await shot('bows-ce-asks');
    expect(describe(await axeViolations(page)), 'axe bow asks').toBe('');

    // A bow asks for an arrow set, never a caliber; nothing is chosen for the user.
    await chooseAmmoSet(page, ce, 'AmmoSet_Arrow');
    const projectile = ce.getByRole('combobox', { name: /Which projectile of the ammo set/ });
    await expect(projectile).toBeEnabled();
    await projectile.selectOption({ index: 1 });
    const tag = ce.getByRole('combobox', { name: /Which bow tag/ });
    const tagLabel = (await tag.locator('option').allTextContents()).find((t) => /CE_Bow/.test(t)) ?? '';
    await tag.selectOption({ label: tagLabel });
    // the estimates are taken one by one, as the page says
    const uses = ce.getByRole('button', { name: /^Use estimate / });
    for (let left = await uses.count(); left > 0; left -= 1) {
      await uses.first().click();
      await expect(uses).toHaveCount(left - 1);
    }
    await expect(ce.getByRole('heading', { name: /answers? still needed/ })).toBeHidden({ timeout: 30_000 });
    await shot('bows-ce-answered');

    const output = page.getByRole('region', { name: 'Output', exact: true });
    await expect(output.getByRole('button', { name: /^Preview Compat\/CombatExtended\// })).toBeVisible({ timeout: 30_000 });
    await expect(page.getByRole('region', { name: 'Diagnostics' }).getByText('No errors')).toBeVisible();
    await expect(output.getByRole('button', { name: 'Apply to project' })).toBeEnabled();
    await output.getByRole('button', { name: 'Apply to project' }).click();
    const dialog = page.getByRole('dialog', { name: 'Apply to project' });
    await dialog.getByRole('button', { name: /^Write [0-9]+ files?/ }).click();
    await expect(dialog.getByText(/Wrote [0-9]+ files?/)).toBeVisible({ timeout: 60_000 });
    await shot('bows-apply-report');
    await dialog.getByRole('button', { name: /Close|Done/ }).first().click();

    const files = walk(work);
    const patchPath = files.find((f) => f.startsWith(join('Compat', 'CombatExtended')) && f.endsWith('.xml'));
    expect(patchPath, `files: ${files.join(', ')}`).toBeDefined();
    const patch = readFileSync(join(work, patchPath ?? ''), 'utf8');
    expect(patch).toContain('Bow_QaShort');
    expect(patch).toContain('AmmoSet_Arrow');
    expect(patch).toContain('CE_Bow');
    expect(patch).toContain('CombatExtended.Verb_ShootCE');
    // a bow has no magazine
    expect(patch).not.toContain('magazineSize');
    const vanilla = readFileSync(join(work, 'Defs', 'ThingDefs_Misc', 'Weapons', 'RangedNeolithic', 'Bow_QaShort.xml'), 'utf8');
    expect(vanilla).not.toContain('CombatExtended');

  });

  test('a weapon platform with an attachment, Combat Extended on', async ({ page, env, shot }) => {
    const work = join(env.tmp, 'work-bows', 'Bows Mod');
    await gotoRoute(page, env, '/weapons');
    await openWeaponsOn(page, work);
    await cloneReference(page, 'bolt-action', 'Gun_QaPlatform');
    const ce = page.getByRole('region', { name: 'Combat Extended', exact: true });
    await ce.getByRole('switch', { name: 'Add a Combat Extended patch (optional)' }).click();
    await expect(ce.getByRole('heading', { name: /answers? still needed/ })).toBeVisible({ timeout: 30_000 });
    const platform = ce.locator('details', { hasText: 'Weapon platform' }).first();
    await platform.locator('summary').click();
    await platform.getByRole('switch', { name: 'This weapon is a platform' }).click();
    await platform.getByRole('button', { name: 'Add an attachment' }).click();
    await platform.getByRole('textbox', { name: 'Attachment definition' }).fill('QaAttachment_Scope');
    await shot('platform-block');

    // the caliber is the user's choice
    await chooseAmmoSet(page, ce, 'AmmoSet_338Lapua');
    await ce.getByRole('combobox', { name: /Which projectile of the ammo set/ }).selectOption({ index: 1 });
    const tag = ce.getByRole('combobox', { name: /Which weapon class tag/ });
    await expect(tag.locator('option')).not.toHaveCount(1);
    await tag.selectOption({ index: 1 });
    await ce.getByRole('radio', { name: 'All derived' }).check();
    const uses = ce.getByRole('button', { name: /^Use( estimate)? [0-9.]+$/ });
    for (let left = await uses.count(); left > 0; left -= 1) {
      await uses.first().click();
      await expect(uses).toHaveCount(left - 1);
    }
    await expect(ce.getByRole('heading', { name: /answers? still needed/ })).toBeHidden({ timeout: 30_000 });
    const output = page.getByRole('region', { name: 'Output', exact: true });
    await expect(output.getByRole('button', { name: /^Preview Compat\/CombatExtended\// })).toBeVisible({ timeout: 30_000 });
    await shot('platform-planned');
    expect(describe(await axeViolations(page)), 'axe platform block').toBe('');
    // the unknown attachment is reported by the plan, not silently dropped
    const diagnostics = page.getByRole('region', { name: 'Diagnostics' });
    await expect(diagnostics.getByText('No errors')).toBeVisible();
    await expect(output.getByRole('button', { name: 'Apply to project' })).toBeEnabled();
    await output.getByRole('button', { name: 'Apply to project' }).click();
    const dialog = page.getByRole('dialog', { name: 'Apply to project' });
    await dialog.getByRole('button', { name: /^Write [0-9]+ files?/ }).click();
    await expect(dialog.getByText(/Wrote [0-9]+ files?/)).toBeVisible({ timeout: 60_000 });
    await shot('platform-apply-report');
    await dialog.getByRole('button', { name: /Close|Done/ }).first().click();

    const patches = walk(work).filter((f) => f.startsWith(join('Compat', 'CombatExtended')) && f.endsWith('.xml'));
    const text = patches.map((f) => readFileSync(join(work, f), 'utf8')).join('\n');
    expect(text).toContain('Gun_QaPlatform');
    // the platform members ride in the conversion operation of the weapon
    expect(text).toContain('<isWeaponPlatform>true</isWeaponPlatform>');
    expect(text).toContain('<attachment>QaAttachment_Scope</attachment>');
    const vanilla = readFileSync(join(work, 'Defs', 'ThingDefs_Misc', 'Weapons', 'RangedIndustrial', 'Gun_QaPlatform.xml'), 'utf8');
    expect(vanilla).not.toContain('QaAttachment_Scope');
  });

  test('the Patches page scans a mod with a bow and a platform candidate', async ({ page, env, shot }) => {
    // the vanilla definitions written above, without their patches, in a mod of their own
    const source = join(env.tmp, 'work-bows', 'Bows Mod');
    const scan = emptyMod(join(env.tmp, 'work-bows'), 'Bows Scan', 'qa.bows.scan');
    cpSync(join(source, 'Defs'), join(scan, 'Defs'), { recursive: true });
    await rememberProject(page, scan);
    await gotoRoute(page, env, '/patches');
    const table = page.getByRole('grid', { name: 'Weapons of the mod' });
    await expect(table.getByRole('row')).toHaveCount(3, { timeout: 60_000 });
    const bow = table.getByRole('row', { name: /Bow_QaShort/ });
    await expect(bow).toBeVisible();
    await expect(bow).toContainText('bow');
    await bow.click();
    const form = page.getByRole('tabpanel', { name: /^Questions/ });
    await expect(form.getByRole('button', { name: 'Browse all ammo' })).toBeVisible();
    await shot('bows-patches-scan');
    expect(describe(await axeViolations(page)), 'axe bow scan').toBe('');
    await page.getByRole('tab', { name: 'Options' }).click();
    await shot('bows-patches-options');
    expect(describe(await axeViolations(page)), 'axe options tab').toBe('');
    await page.getByRole('tab', { name: /^Questions/ }).click();
    // bows are convertible: the questions are about arrow sets and the options tab is there
    await expect(bow).toContainText('Not converted');
    await expect(page.getByRole('tab', { name: 'Options' })).toBeVisible();
    await chooseAmmoSet(page, form, 'AmmoSet_Arrow');
    const uses = form.getByRole('button', { name: /^Use [0-9.]+$/ });
    for (let left = await uses.count(); left > 0; left -= 1) {
      await uses.first().click({ timeout: 5000 });
      await expect(uses).toHaveCount(left - 1);
    }
    await expect(form.getByText(/^[0-9]+ of [0-9]+ answered/)).toBeVisible();
    await page.getByRole('tab', { name: 'Plan' }).click();
    await expect(page.getByText(/Compat\/CombatExtended|Patches\//).first()).toBeVisible({ timeout: 30_000 });
    await shot('bows-patches-plan');
    await page.getByRole('checkbox', { name: 'Convert short bow copy', exact: true }).check();
    await page.getByRole('button', { name: 'Convert 1 weapon' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: /^(Write|Apply|Convert)/ }).last().click();
    await expect(dialog.getByText(/written|Wrote|Applied|Done/i).first()).toBeVisible({ timeout: 90_000 });
    await shot('bows-patches-report');
    await dialog.getByRole('button', { name: /Close|Done/ }).first().click();
    const files = walk(scan).filter((f) => f.startsWith(join('Compat', 'CombatExtended')) && f.endsWith('.xml'));
    expect(files.length).toBeGreaterThan(0);
    const text = files.map((f) => readFileSync(join(scan, f), 'utf8')).join('\n');
    expect(text).toContain('Bow_QaShort');
    expect(text).toContain('CombatExtended.Verb_ShootCE');
    expect(text).not.toContain('magazineSize');
    // the scan now reads the bow as converted
    await page.getByRole('button', { name: 'Scan again' }).click();
    await expect(page.getByText('1 already CE')).toBeVisible({ timeout: 60_000 });
    await shot('bows-patches-rescan');
  });
});
