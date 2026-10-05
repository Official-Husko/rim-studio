import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { outputEntry, setupOutput, type OutputSetup } from '../../output-testSupport';
import { settle } from '../../testSupport';
import { OutputPanel } from './OutputPanel';

function Harness({ setup }: { setup: OutputSetup }) {
  return (
    <OutputPanel
      draftId={setup.editor.entryId.value}
      projectPath="/mods/OutMod"
      store={setup.output}
      editor={setup.editor}
    />
  );
}

async function show(name: 'vanilla' | 'ce-on' | 'ce-answered') {
  const setup = setupOutput();
  setup.editor.open(outputEntry(name));
  renderWithProviders(<Harness setup={setup} />);
  await setup.flush();
  return setup;
}

describe('OutputPanel', () => {
  it('asks for a draft when none is open', () => {
    const setup = setupOutput();
    renderWithProviders(
      <OutputPanel
        draftId={undefined}
        projectPath={undefined}
        store={setup.output}
        editor={setup.editor}
      />,
    );
    expect(screen.getByText('Open a draft to see its output.')).toBeTruthy();
  });

  it('shows only the vanilla file until the switch is turned on', async () => {
    await show('vanilla');
    expect(screen.getByText('1 file')).toBeTruthy();
    expect(screen.getByText('Gun_OutRifle.xml')).toBeTruthy();
    expect(screen.queryByText('Combat Extended patch')).toBeNull();
    expect(screen.queryByText('LoadFolders.xml')).toBeNull();
    expect(screen.getByRole('switch').getAttribute('aria-checked')).toBe('false');
    expect(screen.getByRole('region', { name: 'XML of Gun_OutRifle.xml' })).toBeTruthy();
    expect(
      (screen.getByRole('button', { name: 'Apply to project' }) as HTMLButtonElement).disabled,
    ).toBe(false);
  });

  it('shows the warnings of the plan with their codes', async () => {
    await show('vanilla');
    expect(screen.getByText('design.duplicate-capacity')).toBeTruthy();
    expect(screen.queryByText('design.texture-shared')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Show 2 notes' }));
    expect(screen.getAllByText('design.texture-shared')).toHaveLength(2);
    fireEvent.click(screen.getByRole('button', { name: 'Hide 2 notes' }));
    expect(screen.queryByText('design.texture-shared')).toBeNull();
  });

  it('blocks Apply while Combat Extended has errors and keeps the patch out of the list', async () => {
    await show('ce-on');
    expect(screen.getByText('7 errors')).toBeTruthy();
    expect(
      (screen.getByRole('button', { name: 'Apply to project' }) as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(
      screen.getByText('The Combat Extended patch files appear once every answer is given.'),
    ).toBeTruthy();
    expect(screen.queryByText('Compat/CombatExtended/Patches')).toBeNull();
  });

  it('jumps from a problem to the control of its Combat Extended answer', async () => {
    await show('ce-on');
    const buttons = screen.getAllByRole('button', { name: 'Go to field' });
    fireEvent.click(buttons[0] as HTMLElement);
    const holder = document.activeElement?.closest('[data-ce-field]');
    expect(holder?.getAttribute('data-ce-field')).toBe('/ce/bulk');
  });

  it('jumps from a problem of the form to its field in the editor', async () => {
    await show('vanilla');
    const field = document.createElement('div');
    field.dataset.field = '/tools/1/capacities';
    const input = document.createElement('input');
    field.append(input);
    document.body.append(field);
    fireEvent.click(screen.getAllByRole('button', { name: 'Go to field' })[0] as HTMLElement);
    expect(document.activeElement).toBe(input);
    field.remove();
  });

  it('shows the patch files, the load folders edit, the lint and the derived values once answered', async () => {
    const setup = await show('ce-answered');
    fireEvent.click(screen.getByRole('radio', { name: 'All derived' }));
    await setup.flush();
    expect(screen.getByText('3 files')).toBeTruthy();
    expect(screen.getAllByText('Combat Extended patch').length).toBeGreaterThan(0);
    expect(screen.getByText('LoadFolders.xml')).toBeTruthy();
    expect(screen.getByText('Combat Extended lint')).toBeTruthy();
    expect(screen.getByText('Derived values')).toBeTruthy();
    expect(
      (screen.getByRole('button', { name: 'Apply to project' }) as HTMLButtonElement).disabled,
    ).toBe(false);
  });

  it('runs the whole flow: turn Combat Extended on, answer, apply, see the result', async () => {
    const setup = await show('vanilla');
    fireEvent.click(screen.getByRole('switch'));
    await setup.flush();
    fireEvent.click(screen.getByRole('radio', { name: 'All derived' }));
    fireEvent.click(screen.getByRole('button', { name: /^AmmoSet_280British/ }));
    fireEvent.change(screen.getByRole('combobox', { name: /Which weapon class tag/ }), {
      target: { value: 'CE_AI_SR' },
    });
    for (const [name, value] of [
      [/CE shot spread/, '0.165'],
      [/CE magazine size/, '7'],
      [/Blunt penetration of the tool stock/, '2.56'],
      [/Blunt penetration of the tool barrel/, '2.56'],
    ] as const) {
      const input = screen.getByRole('spinbutton', { name });
      fireEvent.input(input, { target: { value } });
      fireEvent.blur(input);
    }
    await setup.flush();
    expect(setup.output.plan.value?.files).toHaveLength(3);
    fireEvent.click(screen.getByRole('button', { name: 'Apply to project' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Write 3 files', hidden: true }));
    await settle();
    expect(await screen.findByText('Every file of the plan was written.')).toBeTruthy();
  });
});
