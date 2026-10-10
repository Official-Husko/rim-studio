import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { afterEach, describe, expect, it } from 'vitest';
import { outputEntry, setupOutput } from '../../output-testSupport';
import { ApplyBar } from './ApplyBar';

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  stop = undefined;
});

async function show(name: 'vanilla' | 'ce-on') {
  const setup = setupOutput();
  stop = setup.output.start();
  setup.editor.open(outputEntry(name));
  renderWithProviders(<ApplyBar store={setup.output} onGoTo={() => undefined} />);
  return setup;
}

describe('ApplyBar', () => {
  it('waits for the plan before Apply is possible', async () => {
    const setup = await show('vanilla');
    const button = screen.getByRole('button', { name: 'Apply to project' }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(screen.getByText('Waiting for the plan to update.')).toBeTruthy();
    await setup.flush();
    expect(button.disabled).toBe(false);
  });

  it('blocks Apply while the plan has errors and says how many', async () => {
    const setup = await show('ce-on');
    await setup.flush();
    const button = screen.getByRole('button', { name: 'Apply to project' }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    // the answers the plan waits for are named instead of counted
    expect(screen.getByRole('region', { name: /answers are missing/ })).toBeTruthy();
    expect(screen.getAllByRole('button', { name: 'Go to' }).length).toBeGreaterThan(0);
    fireEvent.click(button);
    expect(setup.output.apply.value.phase).toBe('idle');
  });

  it('opens the confirmation when the plan is clean', async () => {
    const setup = await show('vanilla');
    await setup.flush();
    fireEvent.click(screen.getByRole('button', { name: 'Apply to project' }));
    expect(setup.output.apply.value.phase).toBe('confirm');
  });

  it('notes the last write and reopens its result', async () => {
    const setup = await show('vanilla');
    await setup.flush();
    setup.output.openApply();
    await setup.output.confirmApply({ backup: true, dryApply: false });
    setup.output.closeApply();
    expect(screen.getByText('Wrote 3 files')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Show result' }));
    expect(setup.output.apply.value.phase).toBe('done');
  });
});
