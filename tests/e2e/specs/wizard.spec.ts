import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { axeViolations, describe } from '../support/axe.ts';
import { openWeaponsOn } from '../support/designer.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';

test.describe.configure({ mode: 'serial' });

/** The first file with this name under a folder, or undefined. */
function findFile(root: string, name: string): string | undefined {
  for (const entry of readdirSync(root)) {
    const full = join(root, entry);
    if (statSync(full).isDirectory()) {
      const found = findFile(full, name);
      if (found) return found;
    } else if (entry === name) return full;
  }
  return undefined;
}

test.describe('weapons: the new weapon wizard', () => {
  test('rifle, sniper, descriptors, numbers, verdict, create, tune again and write the file', async ({
    page,
    env,
    shot,
  }) => {
    const work = join(env.tmp, 'work', 'Wizard Mod');
    mkdirSync(join(work, 'About'), { recursive: true });
    writeFileSync(
      join(work, 'About', 'About.xml'),
      '<?xml version="1.0" encoding="utf-8"?>\n<ModMetaData><name>Wizard Mod</name><author>qa</author>' +
        '<packageId>qa.wizard</packageId><supportedVersions><li>1.6</li></supportedVersions>' +
        '<description>qa</description></ModMetaData>\n',
    );
    await gotoRoute(page, env, '/weapons');
    await openWeaponsOn(page, work);

    // 1. type: a family, then the type
    await page.getByRole('button', { name: 'New weapon wizard' }).click();
    const wizard = page.getByRole('dialog', { name: 'New weapon' });
    await expect(wizard.getByRole('button', { name: 'Sniper rifle', exact: true })).toBeVisible({
      timeout: 30_000,
    });
    await expect(wizard.getByRole('region', { name: 'Melee' })).toBeVisible();
    await wizard.getByRole('button', { name: 'Rifle', exact: true }).click();
    await wizard.getByRole('button', { name: 'Sniper rifle', exact: true }).click();
    expect(describe(await axeViolations(page, 'dialog')), 'axe type step').toBe('');
    await shot('wizard-1-type');
    await expect(wizard.getByRole('button', { name: 'Next', exact: true })).toBeEnabled();
    await wizard.getByRole('button', { name: 'Next', exact: true }).click();

    // 2. describe: only what applies, a rate of fire slider with its effect, live numbers
    const summary = wizard.getByLabel('Headline numbers');
    await expect(summary).toContainText('Damage', { timeout: 30_000 });
    await expect(wizard.getByRole('region', { name: 'Calibre' })).toBeVisible();
    const effect = wizard.getByRole('status').filter({ hasText: 'rounds per minute' });
    const before = await effect.innerText();
    const cooldownBefore = await summary.innerText();
    const slider = wizard.getByRole('slider', { name: 'Rounds per minute' });
    await slider.focus();
    for (let i = 0; i < 5; i += 1) await slider.press('ArrowRight');
    await expect(effect).not.toHaveText(before);
    await wizard.getByRole('radio', { name: 'Large' }).click();
    await expect.poll(() => summary.innerText(), { timeout: 20_000 }).not.toBe(cooldownBefore);
    expect(describe(await axeViolations(page, 'dialog')), 'axe describe step').toBe('');
    await shot('wizard-2-describe');
    await wizard.getByRole('button', { name: 'Next', exact: true }).click();

    // 3. numbers: groups with reasons and the derived chip, the verdict, the comparison, a new target
    const damage = wizard.getByRole('list', { name: 'Damage and penetration' });
    await expect(damage).toBeVisible({ timeout: 30_000 });
    await expect(damage.getByText('Derived').first()).toBeVisible();
    await expect(wizard.getByRole('progressbar', { name: 'Typicality' })).toBeVisible();
    await expect(wizard.getByText(/typical|plausible|unusual/).first()).toBeVisible();
    await expect(wizard.getByRole('grid', { name: 'Compared with your install' })).toBeVisible();
    await wizard.getByRole('radio', { name: 'Stronger' }).click();
    await expect(wizard.getByText(/^was /).first()).toBeVisible({ timeout: 20_000 });
    expect(describe(await axeViolations(page, 'dialog')), 'axe wizard').toBe('');
    await shot('wizard-3-numbers');
    await wizard.getByRole('button', { name: 'Next', exact: true }).click();

    // 4. name and create: Combat Extended stays off
    await wizard.getByRole('textbox', { name: 'Def name prefix' }).fill('QA_');
    await wizard.getByRole('textbox', { name: 'Label' }).fill('Wizard Sniper');
    await expect(wizard.getByRole('textbox', { name: /Def name$/ })).toHaveValue('QA_WizardSniper');
    await expect(wizard.getByText(/The Combat Extended patch is off/)).toBeVisible();
    expect(describe(await axeViolations(page, 'dialog')), 'axe name step').toBe('');
    await shot('wizard-4-name');
    await wizard.getByRole('button', { name: 'Create' }).click();

    // the draft opens in the editor with the proposed numbers, editable, with the suggested chip
    await expect(wizard).toBeHidden({ timeout: 30_000 });
    await expect(
      page.getByRole('heading', { level: 2, name: 'Wizard Sniper' }).first(),
    ).toBeVisible();
    await expect(
      page.getByText('The numbers of this draft were proposed for rifle/sniper.'),
    ).toBeVisible();
    const field = page.getByRole('spinbutton', { name: 'Damage', exact: true });
    await expect(field).not.toHaveValue('');
    expect(Number(await field.inputValue())).toBeGreaterThan(10);
    await expect(page.getByText('Suggested').first()).toBeVisible();
    await shot('wizard-5-editor');

    // type a number, then propose again: the typed number stays
    await field.fill('33');
    await field.blur();
    await page.getByRole('button', { name: 'Propose again' }).click();
    const again = page.getByRole('dialog', { name: 'Propose the numbers again' });
    await expect(again.getByRole('region', { name: 'Rate of fire' })).toBeVisible({
      timeout: 30_000,
    });
    await again.getByRole('button', { name: 'Next', exact: true }).click();
    await expect(again.getByText(/You typed this number, so it stays/).first()).toBeVisible({
      timeout: 20_000,
    });
    await shot('wizard-6-retune');
    await again.getByRole('button', { name: 'Apply to draft' }).click();
    await expect(again).toBeHidden({ timeout: 30_000 });
    await expect(field).toHaveValue('33');

    // apply to the project and check the file: vanilla, no Combat Extended output
    const output = page.getByRole('region', { name: 'Output', exact: true });
    await expect(output.getByRole('button', { name: /^Preview Defs\// }).first()).toBeVisible({
      timeout: 30_000,
    });
    await output.getByRole('button', { name: 'Apply to project' }).click();
    const apply = page.getByRole('dialog', { name: 'Apply to project' });
    await expect(apply.getByRole('list', { name: 'Files to write' })).toContainText(
      'QA_WizardSniper.xml',
    );
    await apply.getByRole('button', { name: /^Write \d+ file/ }).click();
    await expect(apply.getByText(/^Wrote \d+ file/)).toBeVisible({ timeout: 30_000 });
    await apply
      .getByRole('button', { name: /Close|Done/ })
      .first()
      .click();

    const written = findFile(work, 'QA_WizardSniper.xml');
    expect(written, 'the written weapon file').toBeDefined();
    const xml = readFileSync(written ?? '', 'utf8');
    expect(xml).toContain('<defName>QA_WizardSniper</defName>');
    expect(xml).toContain('<damageAmountBase>33</damageAmountBase>');
    expect(xml).toContain('Bullet_QA_WizardSniper');
    expect(xml).toContain('ParentName=');
    expect(existsSync(join(work, 'Compat'))).toBe(false);
    expect(existsSync(join(work, 'LoadFolders.xml'))).toBe(false);
  });

  test('a melee type shows only the descriptors that apply, and a thin class says so', async ({
    page,
    env,
    shot,
  }) => {
    const work = join(env.tmp, 'work', 'Wizard Mod');
    await gotoRoute(page, env, '/weapons');
    await openWeaponsOn(page, work);
    await page.getByRole('button', { name: 'New weapon wizard' }).click();
    const wizard = page.getByRole('dialog', { name: 'New weapon' });
    await wizard.getByRole('button', { name: 'Sword', exact: true }).click();
    await wizard.getByRole('button', { name: 'Long sword', exact: true }).click();
    await wizard.getByRole('button', { name: 'Next', exact: true }).click();
    await expect(wizard.getByRole('region', { name: 'Swing speed' })).toBeVisible({
      timeout: 30_000,
    });
    await expect(wizard.getByRole('region', { name: 'Calibre' })).toHaveCount(0);
    await expect(wizard.getByRole('region', { name: 'Action' })).toHaveCount(0);
    await wizard.getByRole('radio', { name: 'Fast' }).click();
    await wizard.getByRole('button', { name: 'Next', exact: true }).click();
    await expect(wizard.getByRole('list', { name: 'Attacks' })).toBeVisible({ timeout: 30_000 });
    await expect(wizard.getByText('Rough comparison')).toBeVisible();
    await shot('wizard-7-melee-numbers');
    await wizard.getByRole('button', { name: 'Cancel' }).click();
    await expect(wizard).toBeHidden();
  });

  test('a real Combat Extended calibre is optional and leaves the patch switched off', async ({
    page,
    env,
    shot,
  }) => {
    const work = join(env.tmp, 'work', 'Wizard Mod');
    await gotoRoute(page, env, '/weapons');
    await openWeaponsOn(page, work);
    await page.getByRole('button', { name: 'New weapon wizard' }).click();
    const wizard = page.getByRole('dialog', { name: 'New weapon' });
    await wizard.getByRole('button', { name: 'Assault rifle', exact: true }).click();
    await wizard.getByRole('button', { name: 'Next', exact: true }).click();
    await wizard
      .getByRole('switch', { name: 'Use a calibre from my Combat Extended ammo' })
      .click();
    const ammo = wizard.getByRole('combobox', { name: 'Combat Extended ammo set' });
    await ammo.click();
    await ammo.fill('7.62x39');
    await wizard.getByRole('option').first().click();
    await expect(wizard.getByLabel('Your choices')).toContainText('7.62x39', { timeout: 30_000 });
    expect(describe(await axeViolations(page, 'dialog')), 'axe calibre list').toBe('');
    await shot('wizard-8-ce-calibre');
    await wizard.getByRole('button', { name: 'Next', exact: true }).click();
    await expect(wizard.getByRole('list', { name: 'Damage and penetration' })).toBeVisible({
      timeout: 30_000,
    });
    await expect(wizard.getByText(/ammo set x/).first()).toBeVisible();
    await wizard.getByRole('button', { name: 'Next', exact: true }).click();
    await wizard.getByRole('textbox', { name: 'Label' }).fill('Wizard Assault');
    await wizard.getByRole('textbox', { name: /Def name$/ }).fill('QA_WizardAssault');
    await wizard.getByRole('button', { name: 'Create' }).click();
    await expect(wizard).toBeHidden({ timeout: 30_000 });
    await expect(
      page.getByRole('heading', { level: 2, name: 'Wizard Assault' }).first(),
    ).toBeVisible();
    const patch = page.getByRole('switch', { name: 'Add a Combat Extended patch (optional)' });
    await expect(patch).toBeVisible();
    await expect(patch).not.toBeChecked();
  });
});
