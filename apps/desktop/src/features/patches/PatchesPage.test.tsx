import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { loadFixture, mockError, renderWithProviders } from 'rimstudio-testkit';
import type { ConvertScanDto } from 'rimstudio-ipc-types';
import { beforeEach, describe, expect, it } from 'vitest';
import { FolderPickerHost } from '~/shared/platform';
import { currentProject } from '~/shared/project';
import PatchesPage from './PatchesPage';
import { installTransport, openFixtureProject } from './testSupport';

beforeEach(() => {
  window.location.hash = '#/patches';
});

function renderPage() {
  return renderWithProviders(
    <>
      <PatchesPage />
      <FolderPickerHost />
    </>,
  );
}

describe('PatchesPage without a project', () => {
  it('asks for a mod folder', () => {
    installTransport();
    renderPage();
    expect(screen.getByRole('heading', { name: 'Patches' })).toBeTruthy();
    expect(screen.getByText('No mod is open')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Choose a mod folder' })).toBeTruthy();
  });
});

describe('PatchesPage scan', () => {
  it('lists the weapons with their status, kind and open questions', async () => {
    installTransport();
    openFixtureProject();
    renderPage();
    const table = await screen.findByRole('grid', { name: 'Weapons of the mod' });
    expect(within(table).getAllByText('Not converted')).toHaveLength(5);
    expect(within(table).getByText('OH_G41w_25r_sniper')).toBeTruthy();
    expect(within(table).getAllByText('Ranged / SniperRifle')).toHaveLength(2);
    expect(screen.getByText('5 not converted')).toBeTruthy();
    expect(screen.getByText('0 already CE')).toBeTruthy();
  });

  it('shows already converted weapons without a pick box', async () => {
    installTransport({ designer_convert_scan: () => loadFixture('patches-scan-converted') });
    openFixtureProject('patches-project-converted');
    renderPage();
    const table = await screen.findByRole('grid', { name: 'Weapons of the mod' });
    expect(within(table).getAllByText('Already CE')).toHaveLength(5);
    expect(within(table).queryAllByRole('checkbox')).toHaveLength(0);
    expect(screen.getByRole('button', { name: 'Convert 0 weapons' }).hasAttribute('disabled')).toBe(
      true,
    );
  });

  it('shows unsupported kinds and missing targets with their reason', async () => {
    const scan = loadFixture<ConvertScanDto>('designer_convert_scan');
    const [a, b, ...rest] = scan.candidates;
    if (!a || !b) throw new Error('fixture');
    const synthetic: ConvertScanDto = {
      ...scan,
      candidates: [
        { ...a, status: 'unsupported-kind', reason: 'a bow is not supported', asks: [] },
        {
          ...b,
          status: 'target-not-found',
          reason: 'the target definition was not found',
          asks: [],
        },
        ...rest.slice(0, 1),
      ],
      counts: { notConverted: 1, alreadyCe: 0, unsupportedKind: 1, targetNotFound: 1 },
    };
    installTransport({ designer_convert_scan: () => synthetic });
    openFixtureProject();
    renderPage();
    expect(await screen.findByText('Unsupported kind')).toBeTruthy();
    expect(screen.getByText('Target not found')).toBeTruthy();
    expect(screen.getByText('1 unsupported')).toBeTruthy();
    fireEvent.click(screen.getByText('OH_G41m'));
    expect(await screen.findByText('a bow is not supported')).toBeTruthy();
    expect(
      screen.getByText('This weapon cannot be converted. The reason is given above.'),
    ).toBeTruthy();
  });

  it('says so when the mod has no weapons', async () => {
    installTransport({
      designer_convert_scan: () => ({
        candidates: [],
        counts: { notConverted: 0, alreadyCe: 0, unsupportedKind: 0, targetNotFound: 0 },
        diagnostics: [],
      }),
    });
    openFixtureProject();
    renderPage();
    expect(await screen.findByText('No weapons found')).toBeTruthy();
  });

  it('explains plainly that Combat Extended is not installed', async () => {
    installTransport({
      designer_convert_scan: () => {
        throw mockError(
          'designer.reference-unavailable',
          'no Combat Extended data is loaded, so weapons cannot be converted',
        );
      },
    });
    openFixtureProject();
    renderPage();
    expect(await screen.findByText('Combat Extended is not available')).toBeTruthy();
    expect(screen.getByText(/no Combat Extended data is loaded/)).toBeTruthy();
    expect(
      screen.getByRole('link', { name: /Combat Extended folder on the Setup page/ }),
    ).toBeTruthy();
  });

  it('reports a project folder that cannot be found', async () => {
    installTransport({
      designer_convert_scan: () => {
        throw mockError('project.not-open', 'the project is not open');
      },
      project_open: () => {
        throw mockError('io.not-found', 'the folder does not exist');
      },
    });
    openFixtureProject();
    renderPage();
    expect(await screen.findByText('The mod folder was not found')).toBeTruthy();
    expect(screen.getByText('the folder does not exist')).toBeTruthy();
  });

  it('opens the project again when the backend forgot it', async () => {
    let first = true;
    const transport = installTransport({
      designer_convert_scan: () => {
        if (first) {
          first = false;
          throw mockError('project.not-open', 'the project is not open');
        }
        return loadFixture('designer_convert_scan');
      },
    });
    openFixtureProject();
    renderPage();
    expect(await screen.findByRole('grid', { name: 'Weapons of the mod' })).toBeTruthy();
    expect(transport.calls.filter((c) => c.name === 'project_open')).toHaveLength(1);
  });

  it('shows any other error with its code and a retry', async () => {
    installTransport({
      designer_convert_scan: () => {
        throw mockError('designer.internal', 'something broke');
      },
    });
    openFixtureProject();
    renderPage();
    expect(await screen.findByText('The scan failed')).toBeTruthy();
    expect(screen.getByText('designer.internal')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeTruthy();
  });
});

describe('PatchesPage conversion', () => {
  it('answers the questions and shows the plan with its derived numbers', async () => {
    const transport = installTransport();
    openFixtureProject();
    renderPage();
    await screen.findByRole('grid', { name: 'Weapons of the mod' });
    expect(await screen.findByText('0 of 9 answered')).toBeTruthy();
    // the ranked ammo sets come first
    const ammo = screen.getByRole('combobox', {
      name: 'Which caliber (ammo set) does the weapon use?',
    });
    fireEvent.focus(ammo);
    await waitFor(() => {
      const first = screen.getAllByRole('option')[0];
      expect(first?.textContent).toContain('AmmoSet_303British');
      expect(first?.textContent).toContain('used by 2 converted weapons');
    });
    fireEvent.keyDown(ammo, { key: 'ArrowDown' });
    fireEvent.keyDown(ammo, { key: 'Enter' });
    await waitFor(() => expect(screen.getByText('1 of 9 answered')).toBeTruthy());
    await waitFor(() => {
      const plans = transport.calls.filter((c) => c.name === 'designer_export_plan');
      expect(JSON.stringify(plans.at(-1)?.request)).toMatch(/"ammoSet":"AmmoSet_/);
    });
    // the plan with the answer is the ready one of the fixtures
    fireEvent.click(screen.getByRole('tab', { name: /Plan/ }));
    expect(await screen.findByText('The plan is ready: 2 files will be written.')).toBeTruthy();
    expect(
      screen.getByText('Compat/CombatExtended/Patches/gewehr41_Weapons_Ranged.xml', {
        selector: 'code',
      }),
    ).toBeTruthy();
    expect(screen.getByRole('grid', { name: 'Derived numbers' })).toBeTruthy();
    expect(screen.getAllByText('reliable').length).toBeGreaterThan(0);
  });

  it('shows the open questions of a plan without files', async () => {
    installTransport();
    openFixtureProject();
    renderPage();
    await screen.findByRole('grid', { name: 'Weapons of the mod' });
    fireEvent.click(screen.getByRole('tab', { name: /Plan/ }));
    expect(await screen.findByText('9 questions are still open')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Go to the questions' }));
    expect(screen.getByText('0 of 9 answered')).toBeTruthy();
  });

  it('answers a whole family at once', async () => {
    const transport = installTransport();
    openFixtureProject();
    renderPage();
    await screen.findByRole('grid', { name: 'Weapons of the mod' });
    fireEvent.click(await screen.findByRole('radio', { name: 'Its family (3 weapons)' }));
    expect(
      screen.getByText(/apply to every weapon of the family ranged\/Gun\/Bullet_MauserRifle/),
    ).toBeTruthy();
    const oneHanded = screen.getByRole('radiogroup', { name: 'Is the weapon held in one hand?' });
    fireEvent.click(within(oneHanded).getByRole('radio', { name: 'No' }));
    await waitFor(() => {
      const plans = transport.calls.filter((c) => c.name === 'designer_export_plan');
      const body = JSON.stringify(plans.at(-1)?.request);
      expect(body).toContain('"groups":[{"family":"ranged/Gun/Bullet_MauserRifle"');
    });
  });

  it('applies the checked weapons after a confirmation that lists the files', async () => {
    const transport = installTransport({
      designer_export_plan: () => loadFixture('patches-plan-ready'),
    });
    openFixtureProject();
    renderPage();
    await screen.findByRole('grid', { name: 'Weapons of the mod' });
    fireEvent.click(screen.getByRole('checkbox', { name: /Select all 5 weapons/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Convert 5 weapons' }));
    const dialog = await screen.findByRole('dialog', { name: 'Apply the conversion' });
    expect(await within(dialog).findByText('5 weapons will be converted.')).toBeTruthy();
    expect(within(dialog).getByText('LoadFolders.xml', { selector: 'code' })).toBeTruthy();
    expect(within(dialog).getByText(/A new LoadFolders.xml is created/)).toBeTruthy();
    expect(
      within(dialog).getByRole('switch', { name: 'Back up files that are replaced' }),
    ).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Write 5 weapons' }));
    expect(await within(dialog).findByText('5 weapons applied')).toBeTruthy();
    expect(transport.calls.filter((c) => c.name === 'designer_apply_plan')).toHaveLength(5);
    const applied = transport.calls.find((c) => c.name === 'designer_apply_plan')?.request as {
      backup: boolean;
      dryApply: boolean;
    };
    expect(applied.backup).toBe(true);
    expect(applied.dryApply).toBe(true);
    fireEvent.click(within(dialog).getAllByRole('button', { name: 'Close' }).at(-1) as HTMLElement);
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('keeps weapons with open questions out of the apply', async () => {
    installTransport();
    openFixtureProject();
    renderPage();
    await screen.findByRole('grid', { name: 'Weapons of the mod' });
    fireEvent.click(screen.getByRole('checkbox', { name: /Select all 5 weapons/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Convert 5 weapons' }));
    const dialog = await screen.findByRole('dialog');
    expect(await within(dialog).findByText('5 weapons cannot be applied yet')).toBeTruthy();
    expect(
      within(dialog).getByRole('button', { name: 'Write 0 weapons' }).hasAttribute('disabled'),
    ).toBe(true);
  });
});

describe('PatchesPage lint', () => {
  it('switches to the lint view', async () => {
    installTransport({
      designer_convert_scan: () => loadFixture('patches-scan-converted'),
      designer_export_plan: () => loadFixture('patches-lint-plan'),
    });
    openFixtureProject('patches-project-converted');
    renderPage();
    await screen.findByRole('grid', { name: 'Weapons of the mod' });
    fireEvent.click(screen.getByRole('radio', { name: 'Lint' }));
    expect(
      await screen.findByText('Converted weapons checked: 5. Errors: 0. Warnings: 0.'),
    ).toBeTruthy();
    expect(currentProject.value?.name).toBe("Huskos's Gewehr 41");
  });
});
