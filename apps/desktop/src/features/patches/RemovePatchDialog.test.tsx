import { fireEvent, render, screen, waitFor, within } from '@testing-library/preact';
import { loadFixture, mockError } from 'rimstudio-testkit';
import type {
  ApplyReportDto,
  ConvertCandidateDto,
  ConvertScanDto,
  WritePlanDto,
} from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import { RemovePatchDialog } from './RemovePatchDialog';
import { closeRemoval, openRemoval, removal, removalTarget, type RemovalMode } from './removeStore';
import { installTransport, openFixtureProject } from './testSupport';

const found = loadFixture<ConvertScanDto>('patches-scan-converted').candidates[0];
if (!found) throw new Error('fixture');
// the weapon whose patch RimStudio generated
const generated: ConvertCandidateDto = found;

const PATH = 'Compat/CombatExtended/Patches/gewehr41_Weapons_Ranged.xml';

function setup(extra: Parameters<typeof installTransport>[0] = {}) {
  const transport = installTransport(extra);
  const p = openFixtureProject();
  const project = { projectId: p.projectId, path: p.path, name: p.name };
  render(<RemovePatchDialog project={project} />);
  return { transport, project };
}

async function open(
  project: { projectId: string; path: string; name: string },
  mode: RemovalMode = 'remove',
) {
  const pending = openRemoval(project, removalTarget(generated, mode));
  const title = mode === 'remove' ? 'Remove the patch of' : 'Replace the patch of';
  const dialog = await screen.findByRole('dialog', { name: new RegExp(title) });
  await pending;
  return dialog;
}

describe('RemovePatchDialog', () => {
  it('renders nothing while closed', () => {
    setup();
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('plans, shows the section that goes as a diff, and writes after the confirmation', async () => {
    const { transport, project } = setup();
    await open(project);
    const dialog = screen.getByRole('dialog', { name: 'Remove the patch of Gewehr 41 (m)' });
    expect(
      within(dialog).getByText(/only sections that RimStudio generated are removed/i),
    ).toBeTruthy();
    expect(within(dialog).getByText('1 file will change')).toBeTruthy();
    const file = within(dialog).getByRole('region', { name: PATH });
    expect(within(file).getByText('Section of this weapon removed')).toBeTruthy();
    const diff = within(file).getByRole('region', { name: `Changes to ${PATH}` });
    expect(diff.querySelectorAll('[data-kind="remove"]').length).toBeGreaterThan(5);
    expect(diff.textContent).toContain('====== OH_G41m ======');
    expect(within(dialog).getByText(/never in the mod folder/)).toBeTruthy();
    expect(
      within(dialog).getByText('The definition file of the weapon is never changed.'),
    ).toBeTruthy();
    expect(within(dialog).queryByText(/questions are asked again/)).toBeNull();

    fireEvent.click(within(dialog).getByRole('button', { name: 'Remove patch' }));
    expect(await within(dialog).findByText('Patch removed')).toBeTruthy();
    expect(within(dialog).getByText('1 file was changed.')).toBeTruthy();
    const applied = transport.calls.find((c) => c.name === 'designer_apply_plan');
    expect(applied?.request).toMatchObject({
      backup: true,
      dryApply: false,
      request: { convert: { defName: 'OH_G41m', remove: true } },
    });
    const report = loadFixture<ApplyReportDto>('patches-apply-remove');
    expect(within(dialog).getByText(report.written[0]?.backupPath ?? '')).toBeTruthy();
    expect(within(dialog).getByText('Checked on disk')).toBeTruthy();
    expect(within(dialog).queryByText(/Answer its questions/)).toBeNull();

    fireEvent.click(within(dialog).getAllByRole('button', { name: 'Close' }).at(-1) as HTMLElement);
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('shows a spinner while the plan is built and while the files are written', async () => {
    const gates = { plan: () => {}, apply: () => {} };
    const planGate = new Promise<void>((resolve) => {
      gates.plan = resolve;
    });
    const applyGate = new Promise<void>((resolve) => {
      gates.apply = resolve;
    });
    const { project } = setup({
      designer_export_plan: async () => {
        await planGate;
        return loadFixture<WritePlanDto>('patches-plan-remove');
      },
      designer_apply_plan: async () => {
        await applyGate;
        return loadFixture<ApplyReportDto>('patches-apply-remove');
      },
    });
    const planning = openRemoval(project, removalTarget(generated, 'remove'));
    const dialog = await screen.findByRole('dialog', { name: 'Remove the patch of Gewehr 41 (m)' });
    expect(
      within(dialog).getByRole('status', { name: 'Finding the generated sections' }),
    ).toBeTruthy();
    expect(within(dialog).queryByRole('button', { name: 'Remove patch' })).toBeNull();
    gates.plan();
    await planning;
    fireEvent.click(await within(dialog).findByRole('button', { name: 'Remove patch' }));
    expect(await within(dialog).findByRole('status', { name: 'Removing the patch' })).toBeTruthy();
    // nothing can be pressed while the files are written
    expect(within(dialog).queryByRole('button', { name: 'Cancel' })).toBeNull();
    gates.apply();
    expect(await within(dialog).findByText('Patch removed')).toBeTruthy();
  });

  it('says a file that holds nothing else is deleted and shows every line going', async () => {
    const { project } = setup({
      designer_export_plan: () => loadFixture<WritePlanDto>('patches-plan-remove-delete'),
    });
    const dialog = await open(project);
    const file = within(dialog).getByRole('region', { name: PATH });
    expect(within(file).getByText('File deleted because nothing else is in it')).toBeTruthy();
    const diff = within(file).getByRole('region', { name: `Changes to ${PATH}` });
    expect(diff.querySelectorAll('[data-kind="add"]')).toHaveLength(0);
    expect(diff.querySelectorAll('[data-kind="remove"]').length).toBeGreaterThan(10);
  });

  it('says the conversion stays when only overrides of a hand written conversion go', async () => {
    const { project } = setup();
    const overrides: ConvertCandidateDto = {
      ...generated,
      existing: { origin: 'project', file: 'Patches/ce_patch.xml', files: [PATH], removable: true },
    };
    const pending = openRemoval(project, removalTarget(overrides, 'remove'));
    const dialog = await screen.findByRole('dialog', { name: 'Remove the patch of Gewehr 41 (m)' });
    await pending;
    expect(within(dialog).getByText(/The conversion itself stays/)).toBeTruthy();
    // a conversion RimStudio generated is taken out whole, so it has no such note
    closeRemoval();
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    await open(project);
    expect(screen.queryByText(/The conversion itself stays/)).toBeNull();
  });

  it('spells out the next step for a replace and writes the same files', async () => {
    const { transport, project } = setup();
    const dialog = await open(project, 'replace');
    expect(within(dialog).getByText(/questions are asked again/)).toBeTruthy();
    expect(within(dialog).queryByRole('button', { name: 'Remove patch' })).toBeNull();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Remove and generate again' }));
    expect(await within(dialog).findByText('Patch removed')).toBeTruthy();
    expect(
      within(dialog).getByText(
        'Gewehr 41 (m) is listed as not converted now. Answer its questions to generate a new patch.',
      ),
    ).toBeTruthy();
    expect(transport.calls.filter((c) => c.name === 'designer_apply_plan')).toHaveLength(1);
  });

  it('lists the diagnostics of a plan with errors and does not let it be confirmed', async () => {
    const { transport, project } = setup();
    const missing = removalTarget({ ...generated, defName: 'OH_Unknown' }, 'remove');
    const pending = openRemoval(project, missing);
    const dialog = await screen.findByRole('dialog');
    await pending;
    expect(within(dialog).getByText('The patch cannot be removed')).toBeTruthy();
    expect(within(dialog).getByText('designer.remove-nothing')).toBeTruthy();
    expect(within(dialog).getByText(/there is nothing to remove/)).toBeTruthy();
    const confirm = within(dialog).getByRole('button', { name: 'Remove patch' });
    expect(confirm.hasAttribute('disabled')).toBe(true);
    fireEvent.click(confirm);
    expect(transport.calls.filter((c) => c.name === 'designer_apply_plan')).toHaveLength(0);
  });

  it('shows the error of a plan that failed and plans again on request', async () => {
    let attempts = 0;
    const { project } = setup({
      designer_export_plan: () => {
        attempts += 1;
        if (attempts === 1) throw mockError('io.not-found', 'the folder does not exist');
        return loadFixture<WritePlanDto>('patches-plan-remove');
      },
    });
    const pending = openRemoval(project, removalTarget(generated, 'remove'));
    const dialog = await screen.findByRole('dialog');
    await pending;
    expect(within(dialog).getByText('The patch was not removed')).toBeTruthy();
    expect(within(dialog).getByText('io.not-found')).toBeTruthy();
    expect(within(dialog).getByText(/the folder does not exist/)).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Try again' }));
    expect(await within(dialog).findByText('1 file will change')).toBeTruthy();
  });

  it('shows the error of a write that failed', async () => {
    const { project } = setup({
      designer_apply_plan: () => {
        throw mockError('designer.plan-stale', 'the patch file changed since the plan was made');
      },
    });
    const dialog = await open(project);
    fireEvent.click(within(dialog).getByRole('button', { name: 'Remove patch' }));
    expect(await within(dialog).findByText('designer.plan-stale')).toBeTruthy();
    expect(within(dialog).getByText(/changed since the plan was made/)).toBeTruthy();
    expect(within(dialog).getByRole('button', { name: 'Try again' })).toBeTruthy();
  });

  it('reports a write that stopped before every file was changed', async () => {
    const stopped: ApplyReportDto = {
      planId: 'p',
      written: [],
      unchanged: [],
      diagnostics: [
        {
          code: 'designer.apply-write-failed',
          severity: 'error',
          message: 'a file could not be written; 0 of 1 files were written before it',
        },
      ],
    };
    const { project } = setup({ designer_apply_plan: () => stopped });
    const dialog = await open(project);
    fireEvent.click(within(dialog).getByRole('button', { name: 'Remove patch' }));
    expect(await within(dialog).findByText('Only part of the patch was removed')).toBeTruthy();
    expect(within(dialog).getByText('designer.apply-write-failed')).toBeTruthy();
    expect(within(dialog).queryByText('Patch removed')).toBeNull();
  });

  it('closes without writing on cancel', async () => {
    const { transport, project } = setup();
    const dialog = await open(project);
    fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(removal.value.phase).toBe('closed');
    expect(transport.calls.filter((c) => c.name === 'designer_apply_plan')).toHaveLength(0);
  });
});
