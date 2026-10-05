import { axeViolations, describe } from '../support/axe.ts';
import { expect, gotoRoute, test } from '../support/fixtures.ts';

const PAGES = [
  { hash: '/setup', heading: 'Setup', name: 'setup' },
  { hash: '/project', heading: 'Mod', name: 'project' },
  { hash: '/weapons', heading: 'Weapons', name: 'weapons' },
  { hash: '/patches', heading: 'Patches', name: 'patches' },
  { hash: '/gallery', heading: 'Gallery', name: 'gallery' },
];

test.describe('every page loads', () => {
  for (const page of PAGES) {
    test(`${page.name} renders without console errors and passes axe`, async ({ page: p, env, shot, problems }) => {
      await gotoRoute(p, env, page.hash);
      await expect(p.locator('main')).toBeVisible();
      // a page chunk compiles on first use in the dev server: wait for the page's own heading
      await expect(p.getByRole('heading', { level: 1 }).first()).toBeVisible({ timeout: 45_000 });
      await p.waitForTimeout(1500);
      if (page.name !== 'gallery') await expect(p.getByRole('alert')).toHaveCount(0);
      await shot(`pages-${page.name}`);
      const violations = await axeViolations(p);
      expect(describe(violations), 'axe violations').toBe('');
      expect(problems.errors).toEqual([]);
    });
  }

  test('the bridge chip reports a working bridge', async ({ page, env }) => {
    await gotoRoute(page, env, '/setup');
    await expect(page.getByText(/bridge ok/i)).toBeVisible();
  });
});
