import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import type { DesignerApplyPlanRequest } from 'rimstudio-ipc-types';
import { mockError } from 'rimstudio-testkit';
import { afterEach, describe, expect, it } from 'vitest';
import { outputEntry, setupOutput } from '../../output-testSupport';
import { settle } from '../../testSupport';
import { ApplyDialog } from './ApplyDialog';

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  stop = undefined;
});

async function open(name: 'vanilla' | 'ce-answered' | 'ce-edited', handlers = {}) {
  const setup = setupOutput(handlers);
  stop = setup.output.start();
  setup.editor.open(outputEntry(name));
  if (name !== 'vanilla') setup.output.setAcceptMode('all');
  await setup.flush();
  renderWithProviders(<ApplyDialog store={setup.output} projectPath="/mods/OutMod" />);
  setup.output.openApply();
  return setup;
}

describe('ApplyDialog', () => {
  it('lists exactly the files to be written and where backups go', async () => {
    await open('ce-answered');
    expect(screen.getByText('These files will be written under /mods/OutMod:')).toBeTruthy();
    const list = screen.getByRole('list', { name: 'Files to write', hidden: true });
    expect(list.querySelectorAll('li')).toHaveLength(3);
    expect(list.textContent).toContain('Compat/CombatExtended/Patches/outmod_Weapons_Ranged.xml');
    expect(list.textContent).toContain('LoadFolders.xml');
    expect(screen.getByText(/Backups are never placed inside the mod folder/)).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Write 3 files', hidden: true })).toBeTruthy();
  });

  it('leaves out unchanged files and says how many', async () => {
    await open('ce-edited');
    const list = screen.getByRole('list', { name: 'Files to write', hidden: true });
    expect(list.querySelectorAll('li')).toHaveLength(1);
    expect(screen.getByText('2 files are already up to date and are left alone.')).toBeTruthy();
  });

  it('writes with the choices of the dialog and shows the result', async () => {
    const setup = await open('ce-answered');
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'Back up files that are replaced', hidden: true }),
    );
    fireEvent.click(screen.getByRole('button', { name: 'Write 3 files', hidden: true }));
    await settle();
    const sent = setup.transport.calls.find((c) => c.name === 'designer_apply_plan')
      ?.request as DesignerApplyPlanRequest;
    expect(sent.backup).toBe(false);
    expect(sent.dryApply).toBe(true);
    expect(await screen.findByText('Wrote 3 files')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Done', hidden: true }));
    expect(setup.output.apply.value.phase).toBe('idle');
  });

  it('only offers the dry run when the plan has a patch', async () => {
    await open('vanilla');
    expect(
      screen.queryByRole('checkbox', { name: /Check the Combat Extended patch/, hidden: true }),
    ).toBeNull();
  });

  it('shows the error of a refused write', async () => {
    await open('vanilla', {
      designer_apply_plan: () => {
        throw mockError('designer.plan-stale', 'The plan changed since you reviewed it.');
      },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Write 1 file', hidden: true }));
    expect(await screen.findByText('designer.plan-stale')).toBeTruthy();
    expect(screen.getByText('The plan changed since you reviewed it.')).toBeTruthy();
  });

  it('shows the progress while the job runs and cannot be cancelled', async () => {
    const setup = await open('vanilla', {
      designer_apply_plan: () => new Promise(() => {}),
    });
    fireEvent.click(screen.getByRole('button', { name: 'Write 1 file', hidden: true }));
    await settle();
    expect(setup.output.apply.value.phase).toBe('running');
    expect(screen.getByRole('progressbar', { name: 'Writing files', hidden: true })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Done', hidden: true })).toBeNull();
  });

  it('is closed when nothing is being applied', () => {
    const setup = setupOutput();
    renderWithProviders(<ApplyDialog store={setup.output} projectPath="/mods/OutMod" />);
    expect(screen.queryByText('Apply to project')).toBeNull();
  });
});
