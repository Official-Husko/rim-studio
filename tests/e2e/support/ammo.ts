import type { Locator, Page } from '@playwright/test';
import { expect } from '@playwright/test';

/**
 * Chooses an ammo set through the browser of the ammo set field: opens it, searches for the def name,
 * selects the row and presses Select this set. `scope` is the region that holds the field.
 */
export async function chooseAmmoSet(page: Page, scope: Locator, def: string): Promise<void> {
  await scope.getByRole('button', { name: 'Browse all ammo' }).click();
  const dialog = page.getByRole('dialog', { name: 'Combat Extended ammunition' });
  await dialog.getByRole('searchbox', { name: 'Search ammo sets' }).fill(def);
  const row = dialog.getByRole('listbox', { name: 'Ammo sets' }).locator(`[data-key="${def}"]`);
  await expect(row).toBeVisible({ timeout: 120_000 });
  await row.click();
  await dialog.getByRole('button', { name: 'Select this set' }).click();
  await expect(dialog).toBeHidden();
}
