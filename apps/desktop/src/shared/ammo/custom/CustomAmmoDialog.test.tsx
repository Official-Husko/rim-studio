import { fireEvent, render, screen, waitFor, within } from '@testing-library/preact';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { CustomAmmoDto, DiagnosticDto, WritePlanDto } from 'rimstudio-ipc-types';
import { CustomAmmoDialog } from './CustomAmmoDialog';
import { createCatalogStore } from '../catalogStore';
import {
  catalogSlice,
  customAmmo,
  customPlan,
  installAmmoTransport,
  suggestionFmj,
} from '../testSupport';

const ERROR: DiagnosticDto = {
  code: 'ce.ammo-field-required',
  severity: 'error',
  message: 'the ammo type AP needs damage',
  field: '/ce/customAmmo/types/1/projectile/damage',
};
const WARNING: DiagnosticDto = {
  code: 'ce.ammo-implausible',
  severity: 'warning',
  message: 'speed of the ammo type FMJ is 9999; the installed ones range from 39 to 20',
  field: '/ce/customAmmo/types/0/projectile/speed',
};

function planOf(diagnostics: DiagnosticDto[], files = customPlan().files) {
  return vi.fn(async (): Promise<WritePlanDto> => ({
    planId: 'p',
    files,
    diagnostics,
    hasErrors: diagnostics.some((d) => d.severity === 'error'),
  }));
}

async function pool() {
  const store = createCatalogStore({ fetchPage: async () => catalogSlice() });
  await store.load();
  return store;
}

function show(props: Partial<Parameters<typeof CustomAmmoDialog>[0]> = {}) {
  const onSave = vi.fn();
  const onClose = vi.fn();
  return pool().then((catalog) => {
    render(
      <CustomAmmoDialog
        open
        initial={customAmmo()}
        plan={planOf([])}
        onSave={onSave}
        onClose={onClose}
        pool={catalog}
        debounceMs={0}
        {...props}
      />,
    );
    return { onSave, onClose };
  });
}

describe('CustomAmmoDialog', () => {
  beforeEach(() => {
    installAmmoTransport();
  });

  it('is shut when it is not open', () => {
    render(<CustomAmmoDialog open={false} onSave={vi.fn()} onClose={vi.fn()} />);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('has the sections of a caliber and starts on the identity', async () => {
    await show({ initial: undefined });
    expect(screen.getByRole('dialog', { name: 'Create custom ammunition' })).toBeTruthy();
    const tabs = screen.getByRole('tablist', { name: 'Sections of the custom ammunition' });
    for (const name of [/^Caliber/, /^Ammo types/, /^Set grouping/, /^Art and sounds/, /^Review/]) {
      expect(within(tabs).getByRole('tab', { name })).toBeTruthy();
    }
    expect(screen.getByRole('textbox', { name: /^Name/ })).toBeTruthy();
    expect(screen.getByText('Add at least one ammo type.')).toBeTruthy();
    expect(
      (screen.getByRole('button', { name: 'Save custom ammo' }) as HTMLButtonElement).disabled,
    ).toBe(true);
  });

  it('shows the checks of the backend under the field they concern and blocks Save on an error', async () => {
    await show({ plan: planOf([ERROR, WARNING]) });
    fireEvent.click(screen.getByRole('tab', { name: /^Ammo types/ }));
    fireEvent.click(await screen.findByRole('button', { name: /^AP/ }));
    expect(await screen.findByText('the ammo type AP needs damage')).toBeTruthy();
    expect(screen.getByRole('button', { name: /^AP/ }).closest('li')?.textContent).toContain(
      'Error',
    );
    expect(screen.getByText('Fix 1 error before saving.')).toBeTruthy();
    expect(
      (screen.getByRole('button', { name: 'Save custom ammo' }) as HTMLButtonElement).disabled,
    ).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: /^FMJ/ }));
    expect(await screen.findByText(WARNING.message)).toBeTruthy();
  });

  it('writes edits into the working copy and saves it', async () => {
    const { onSave } = await show();
    await waitFor(() =>
      expect(
        (screen.getByRole('button', { name: 'Save custom ammo' }) as HTMLButtonElement).disabled,
      ).toBe(false),
    );
    fireEvent.input(screen.getByRole('textbox', { name: /^Name/ }), {
      target: { value: 'Renamed' },
    });
    await waitFor(() =>
      expect(
        (screen.getByRole('button', { name: 'Save custom ammo' }) as HTMLButtonElement).disabled,
      ).toBe(false),
    );
    fireEvent.click(screen.getByRole('button', { name: 'Save custom ammo' }));
    const saved = onSave.mock.calls[0]?.[0] as CustomAmmoDto;
    expect(saved.name).toBe('Renamed');
    expect(saved.types).toHaveLength(2);
  });

  it('edits the numbers of a projectile with the unit, the source and the suggestion', async () => {
    const suggest = vi.fn().mockResolvedValue(suggestionFmj());
    await show({ suggest });
    fireEvent.click(screen.getByRole('tab', { name: /^Ammo types/ }));
    fireEvent.click(await screen.findByRole('button', { name: 'Suggest values' }));
    const damage = (await screen.findByRole('spinbutton', { name: 'Damage' })) as HTMLInputElement;
    fireEvent.input(damage, { target: { value: '15' } });
    fireEvent.blur(damage);
    const holder = damage.closest('[data-ammo-field]') as HTMLElement;
    expect(within(holder).getByText('Typed')).toBeTruthy();
    expect(within(holder).getByText('hp')).toBeTruthy();
    const sharp = screen
      .getByRole('spinbutton', { name: 'Sharp penetration' })
      .closest('[data-ammo-field]') as HTMLElement;
    expect(within(sharp).getByText('Suggested')).toBeTruthy();
    // a typed value that differs from the suggestion offers it, with its rating
    fireEvent.input(screen.getByRole('spinbutton', { name: 'Sharp penetration' }), {
      target: { value: '20' },
    });
    fireEvent.blur(screen.getByRole('spinbutton', { name: 'Sharp penetration' }));
    const note = sharp.querySelector('[data-ammo-suggestion]') as HTMLElement;
    expect(note.textContent).toContain('Suggested 6');
    expect(note.textContent).toContain('Unreliable');
    fireEvent.click(within(note).getByRole('button', { name: 'Use suggestion' }));
    await waitFor(() =>
      expect(
        (screen.getByRole('spinbutton', { name: 'Sharp penetration' }) as HTMLInputElement).value,
      ).toBe('6'),
    );
  });

  it('starts a type from a real ammo type with the copy picker', async () => {
    const suggest = vi
      .fn()
      .mockResolvedValue({ ...suggestionFmj(), copiedFrom: 'Ammo_303British_FMJ' });
    await show({ suggest });
    fireEvent.click(screen.getByRole('tab', { name: /^Ammo types/ }));
    const picker = await screen.findByRole('combobox', { name: 'Copy from existing ammo' });
    fireEvent.focus(picker);
    fireEvent.input(picker, { target: { value: '303' } });
    fireEvent.keyDown(picker, { key: 'ArrowDown' });
    fireEvent.keyDown(picker, { key: 'Enter' });
    await waitFor(() =>
      expect(suggest).toHaveBeenCalledWith(
        expect.objectContaining({ copyFrom: expect.stringContaining('303') }),
      ),
    );
  });

  it('adds, duplicates, moves and removes types', async () => {
    await show({ initial: undefined });
    fireEvent.click(screen.getByRole('tab', { name: /^Ammo types/ }));
    const list = () => screen.getByRole('list', { name: 'Ammo types of the caliber' });
    expect(screen.getByText('No ammo types yet.')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Add an ammo type' }));
    expect(within(list()).getAllByRole('listitem')).toHaveLength(1);
    fireEvent.click(screen.getByRole('button', { name: 'Duplicate' }));
    expect(within(list()).getAllByRole('listitem')).toHaveLength(2);
    expect((screen.getByRole('button', { name: 'Move down' }) as HTMLButtonElement).disabled).toBe(
      true,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Move up' }));
    fireEvent.click(screen.getByRole('button', { name: 'Remove' }));
    expect(within(list()).getAllByRole('listitem')).toHaveLength(1);
  });

  it('edits the recipe: ingredients with a count', async () => {
    await show();
    fireEvent.click(screen.getByRole('tab', { name: /^Ammo types/ }));
    fireEvent.click(await screen.findByRole('tab', { name: 'Crafting' }));
    const group = screen.getByRole('group', { name: 'Ingredients of one craft' });
    fireEvent.click(within(group).getByRole('button', { name: 'Add an ingredient' }));
    fireEvent.input(within(group).getByRole('textbox', { name: 'Ingredient 2' }), {
      target: { value: 'Plasteel' },
    });
    expect(
      (within(group).getByRole('textbox', { name: 'Ingredient 2' }) as HTMLInputElement).value,
    ).toBe('Plasteel');
    fireEvent.click(within(group).getByRole('button', { name: 'Remove ingredient 2' }));
    expect(within(group).queryByRole('textbox', { name: 'Ingredient 2' })).toBeNull();
  });

  it('reviews the files with their XML and the checks, and goes to a field from a check', async () => {
    await show({ plan: planOf([ERROR, WARNING]) });
    fireEvent.click(screen.getByRole('tab', { name: /^Review/ }));
    const files = await screen.findByRole('list', { name: 'Files that will be written' });
    expect(files.textContent).toContain('Compat/CombatExtended/Defs/Ammo/');
    expect(within(files).getByText('Ammunition')).toBeTruthy();
    expect(screen.getByRole('region', { name: /Defs\/Ammo\/.*\.xml/ })).toBeTruthy();
    const go = screen.getAllByRole('button', { name: 'Go to field' });
    expect(go).toHaveLength(2);
    fireEvent.click(go[0] as HTMLElement);
    await waitFor(() =>
      expect(screen.getByRole('tab', { name: /^Ammo types/, selected: true })).toBeTruthy(),
    );
    expect(screen.getByRole('tab', { name: /^Projectile/, selected: true })).toBeTruthy();
  });

  it('says the files cannot be listed while the weapon has problems of its own', async () => {
    await show({
      plan: planOf(
        [
          {
            code: 'design.required-missing',
            severity: 'error',
            message: 'CE magazine size is required',
            field: '/ce/magazineSize',
          },
        ],
        [],
      ),
    });
    fireEvent.click(screen.getByRole('tab', { name: /^Review/ }));
    expect(await screen.findByText('The files cannot be listed yet')).toBeTruthy();
    expect(screen.getByText('CE magazine size is required')).toBeTruthy();
  });

  it('says so when there is no project to check against', async () => {
    await show({ plan: undefined });
    expect(screen.getByText(/No project is open, so the backend cannot check/)).toBeTruthy();
    fireEvent.click(screen.getByRole('tab', { name: /^Review/ }));
    expect(
      screen.getAllByText(/Open a project to see the checks and the files/).length,
    ).toBeGreaterThan(0);
    expect(
      (screen.getByRole('button', { name: 'Save custom ammo' }) as HTMLButtonElement).disabled,
    ).toBe(false);
  });

  it('shows a plan that failed to be made', async () => {
    await show({
      plan: vi.fn(async () => {
        throw { code: 'designer.plan-failed', message: 'The plan failed.', errorId: 'e-1' };
      }),
    });
    expect(await screen.findByText('The plan failed.')).toBeTruthy();
  });

  it('names the art and sounds by path and says importing is not available', async () => {
    await show();
    fireEvent.click(screen.getByRole('tab', { name: /^Art and sounds/ }));
    expect(screen.getByText('Art and sounds are paths for now')).toBeTruthy();
    expect(screen.getAllByRole('textbox', { name: 'Ammo texture' })).toHaveLength(2);
  });

  it('groups the set: similar set and default type', async () => {
    const { onSave } = await show();
    fireEvent.click(screen.getByRole('tab', { name: /^Set grouping/ }));
    fireEvent.change(screen.getByRole('combobox', { name: 'Default type' }), {
      target: { value: 'AP' },
    });
    await waitFor(() =>
      expect(
        (screen.getByRole('button', { name: 'Save custom ammo' }) as HTMLButtonElement).disabled,
      ).toBe(false),
    );
    fireEvent.click(screen.getByRole('button', { name: 'Save custom ammo' }));
    const saved = onSave.mock.calls[0]?.[0] as CustomAmmoDto | undefined;
    expect(saved?.defaultType).toBe('AP');
  });

  it('closes with Cancel and offers Remove only for existing ammunition', async () => {
    const onRemove = vi.fn();
    const { onClose } = await show({ onRemove });
    fireEvent.click(screen.getByRole('button', { name: 'Remove custom ammo' }));
    expect(onRemove).toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onClose).toHaveBeenCalled();
  });
});
