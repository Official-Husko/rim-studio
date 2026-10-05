import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { cloneReference, openWeaponsOn, readout } from '../support/designer.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';

test.describe.configure({ mode: 'serial' });

const DEF = 'Gun_QaRifle';

test.describe('weapons: vanilla clone with its own projectile', () => {
  test('clone, edit the damage, plan, apply and see the files in the project', async ({ page, env, shot }) => {
    const project = join(env.tmp, 'work', 'QA Test Mod');
    await gotoRoute(page, env, '/weapons');
    await openWeaponsOn(page, project);
    await cloneReference(page, 'bolt', DEF);
    await expect(page.getByRole('switch', { name: 'Own projectile' })).toBeChecked();

    const dpsBefore = await readout(page, 'DPS');
    expect(dpsBefore).toBeGreaterThan(0);
    const damage = page.getByRole('spinbutton', { name: 'Damage', exact: true });
    await damage.fill('30');
    await damage.blur();
    await expect.poll(() => readout(page, 'DPS'), { timeout: 20_000 }).toBeGreaterThan(dpsBefore);

    const output = page.getByRole('region', { name: 'Output', exact: true });
    await expect(
      output.getByRole('button', {
        name: `Preview Defs/ThingDefs_Misc/Weapons/RangedIndustrial/${DEF}.xml`,
      }),
    ).toBeVisible();
    await expect(output.getByRole('region', { name: `XML of ${DEF}.xml` })).toContainText('damageAmountBase');
    await shot('weapons-clone-edited');

    await output.getByRole('button', { name: 'Apply to project' }).click();
    const dialog = page.getByRole('dialog', { name: 'Apply to project' });
    await expect(dialog.getByRole('list', { name: 'Files to write' })).toContainText(`${DEF}.xml`);
    await shot('weapons-apply-review');
    await dialog.getByRole('button', { name: /^Write 1 file/ }).click();
    await expect(dialog.getByText('Wrote 1 file')).toBeVisible();
    await shot('weapons-apply-report');
    await dialog.getByRole('button', { name: /Close|Done/ }).first().click();

    const written = join(project, 'Defs', 'ThingDefs_Misc', 'Weapons', 'RangedIndustrial', `${DEF}.xml`);
    expect(existsSync(written)).toBe(true);
    const xml = readFileSync(written, 'utf8');
    expect(xml).toContain(`<defName>${DEF}</defName>`);
    expect(xml).toContain('Bullet_' + DEF);
    expect(xml).toContain('<damageAmountBase>30</damageAmountBase>');
    // vanilla by default: no Combat Extended output
    expect(existsSync(join(project, 'Compat'))).toBe(false);
    expect(existsSync(join(project, 'LoadFolders.xml'))).toBe(false);

    // the Project page shows the written file in its tree
    await page.getByRole('link', { name: 'Project' }).click();
    const tree = page.getByRole('tree');
    await expect(tree).toBeVisible();
    for (const name of ['ThingDefs_Misc', 'Weapons', 'RangedIndustrial']) {
      const item = tree.getByRole('treeitem', { name: new RegExp(`^${name}`) }).first();
      await item.click();
      if ((await item.getAttribute('aria-expanded')) !== 'true') await item.press('ArrowRight');
      await expect(item).toHaveAttribute('aria-expanded', 'true');
    }
    await expect(tree.getByRole('treeitem', { name: new RegExp(`${DEF}\\.xml`) })).toBeVisible();
    await shot('weapons-project-tree');
  });
});
