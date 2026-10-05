import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { catalogSlice, suggestionFmj } from '~/shared/ammo/testSupport';
import { clearAmmoSummaries } from '~/shared/ammo/useAmmoSummary';
import { outputEntry, setupOutput, type OutputSetup } from '../../../output-testSupport';
import { CeSection } from '../CeSection';

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  stop = undefined;
});
beforeEach(() => clearAmmoSummaries());

function Harness({ setup }: { setup: OutputSetup }) {
  const draft = setup.editor.draft.value;
  return draft ? <CeSection store={setup.output} spec={draft.spec} onGoTo={() => {}} /> : null;
}

async function show() {
  const setup = setupOutput(
    {
      designer_ce_ammo_catalog: () => catalogSlice(),
      designer_ce_ammo_suggest: () => suggestionFmj(),
    },
    () => 'p-out',
  );
  stop = setup.output.start();
  setup.editor.open(outputEntry('ce-on'));
  await setup.flush();
  renderWithProviders(<Harness setup={setup} />);
  return setup;
}

describe('the ammo set of the Weapons page', () => {
  it('shows the field with the quick picks instead of a plain list, and nothing chosen for the user', async () => {
    const setup = await show();
    expect(screen.getByRole('button', { name: 'Browse all ammo' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Create custom ammo' })).toBeTruthy();
    expect(screen.getByRole('group', { name: 'Quick picks, best fit first' })).toBeTruthy();
    expect(setup.editor.draft.value?.spec.ce?.ammoSet).toBeUndefined();
    expect(screen.queryByRole('combobox', { name: /Which caliber/ })).toBeNull();
  });

  it('writes the quick pick into the draft and takes the custom ammunition out', async () => {
    const setup = await show();
    fireEvent.click(screen.getByRole('button', { name: /^AmmoSet_280British/ }));
    expect(setup.editor.draft.value?.spec.ce?.ammoSet).toBe('AmmoSet_280British');
    expect(setup.editor.draft.value?.spec.ce?.customAmmo).toBeUndefined();
  });

  it('puts custom ammunition into the block, clears the chosen set and removes it again', async () => {
    const setup = await show();
    fireEvent.click(screen.getByRole('button', { name: /^AmmoSet_280British/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Create custom ammo' }));
    const win = await screen.findByRole('dialog', { name: 'Create custom ammunition' });
    fireEvent.input(within(win).getByRole('textbox', { name: /^Name/ }), {
      target: { value: 'Six' },
    });
    fireEvent.input(within(win).getByRole('textbox', { name: /^Caliber/ }), {
      target: { value: '6mm' },
    });
    fireEvent.click(within(win).getByRole('tab', { name: /^Ammo types/ }));
    fireEvent.click(within(win).getByRole('button', { name: 'Add an ammo type' }));
    await waitFor(() =>
      expect(
        (within(win).getByRole('button', { name: 'Save custom ammo' }) as HTMLButtonElement)
          .disabled,
      ).toBe(false),
    );
    fireEvent.click(within(win).getByRole('button', { name: 'Save custom ammo' }));
    const ce = setup.editor.draft.value?.spec.ce;
    expect(ce?.customAmmo?.name).toBe('Six');
    expect(ce?.ammoSet).toBeUndefined();
    expect(document.querySelector('[data-ammo-custom]')?.textContent).toContain('Six');
    fireEvent.click(screen.getByRole('button', { name: 'Remove custom ammo' }));
    expect(setup.editor.draft.value?.spec.ce?.customAmmo).toBeUndefined();
  });

  it('hides the default projectile question while custom ammunition brings its own', async () => {
    const setup = await show();
    expect(screen.getByRole('combobox', { name: /Which projectile of the ammo set/ })).toBeTruthy();
    setup.output.patchBlock({ customAmmo: { name: 'Six', caliber: '6mm', types: [] } });
    await waitFor(() =>
      expect(
        screen.queryByRole('combobox', { name: /Which projectile of the ammo set/ }),
      ).toBeNull(),
    );
  });
});
