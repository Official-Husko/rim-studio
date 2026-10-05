import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { outputEntry, setupOutput, type OutputSetup } from '../../output-testSupport';
import { fixture } from '../../testSupport';
import { CeSection } from './CeSection';

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  stop = undefined;
});

function Harness({ setup, onGoTo }: { setup: OutputSetup; onGoTo?: (p: string) => void }) {
  const draft = setup.editor.draft.value;
  if (!draft) return null;
  return <CeSection store={setup.output} spec={draft.spec} onGoTo={onGoTo ?? (() => {})} />;
}

async function show(name: 'vanilla' | 'ce-on' | 'ce-answered', handlers = {}, onGoTo?: () => void) {
  const setup = setupOutput(handlers);
  stop = setup.output.start();
  setup.editor.open(outputEntry(name));
  await setup.flush();
  renderWithProviders(<Harness setup={setup} {...(onGoTo ? { onGoTo } : {})} />);
  return setup;
}

describe('CeSection: off', () => {
  it('is off by default and explains the gated folder', async () => {
    await show('vanilla');
    const toggle = screen.getByRole('switch', { name: 'Add a Combat Extended patch (optional)' });
    expect(toggle.getAttribute('aria-checked')).toBe('false');
    expect(
      screen.getByText(/Off by default\. The weapon is always written as a plain RimWorld/),
    ).toBeTruthy();
    expect(screen.getByText(/loads that folder only when Combat Extended is active/)).toBeTruthy();
    expect(screen.getByText(/Turn this on to see the numbers/)).toBeTruthy();
    expect(screen.queryByText('Take derived values')).toBeNull();
  });

  it('turns the block on in the draft only when the switch is used', async () => {
    const setup = await show('vanilla');
    expect(setup.editor.draft.value?.spec.ce).toBeUndefined();
    fireEvent.click(screen.getByRole('switch'));
    expect(setup.editor.draft.value?.spec.ce).toEqual({ oneHanded: false, beltFed: false });
  });

  it('gives the plain reason and keeps the switch off when Combat Extended is not loaded', async () => {
    await show('vanilla', {
      designer_ce_suggest: () => fixture('designer-output-suggest-unavailable'),
    });
    expect(screen.getByText('Combat Extended is not available')).toBeTruthy();
    expect(screen.getByText('no Combat Extended data is loaded')).toBeTruthy();
    const toggle = screen.getByRole('switch') as HTMLButtonElement;
    expect(toggle.disabled).toBe(true);
  });
});

describe('CeSection: on', () => {
  it('shows the suggestion: ratings, the choices and what is still needed', async () => {
    await show('ce-on');
    expect(screen.getByRole('switch').getAttribute('aria-checked')).toBe('true');
    expect(
      screen.getByText('Numbers based on the 4 nearest of 20 converted weapons.'),
    ).toBeTruthy();
    expect(screen.getAllByText('Rough').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Unreliable').length).toBeGreaterThan(0);
    expect(screen.getByRole('combobox', { name: /Which caliber/ })).toBeTruthy();
    expect(screen.getByRole('combobox', { name: /Which weapon class tag/ })).toBeTruthy();
    expect(screen.getByRole('checkbox', { name: 'Is the weapon held in one hand?' })).toBeTruthy();
  });

  it('takes the derived values only after the user asks for them', async () => {
    const setup = await show('ce-on');
    expect(setup.output.accepted.value).toEqual([]);
    expect((screen.getByRole('checkbox', { name: 'Use 7.771' }) as HTMLInputElement).checked).toBe(
      false,
    );
    fireEvent.click(screen.getByRole('radio', { name: 'All derived' }));
    await setup.flush();
    expect(setup.output.acceptMode.value).toBe('all');
    expect((screen.getByRole('checkbox', { name: 'Use 7.771' }) as HTMLInputElement).checked).toBe(
      true,
    );
  });

  it('turns one derived value off and shows the choice as custom', async () => {
    const setup = await show('ce-on');
    fireEvent.click(screen.getByRole('radio', { name: 'All derived' }));
    await setup.flush();
    fireEvent.click(screen.getByRole('checkbox', { name: 'Use 7.771' }));
    expect(setup.output.acceptMode.value).toBe('custom');
    expect(setup.output.accepted.value).not.toContain('/ce/bulk');
    expect(screen.getByRole('radio', { name: 'Custom' }).getAttribute('aria-checked')).toBe('true');
  });

  it('lists the answers the plan waits for and jumps to the control', async () => {
    const onGoTo = vi.fn();
    const setup = await show('ce-on', {}, onGoTo);
    fireEvent.click(screen.getByRole('radio', { name: 'All derived' }));
    await setup.flush();
    expect(screen.getByText('5 answers still needed')).toBeTruthy();
    fireEvent.click(screen.getAllByRole('button', { name: 'Go to' })[1] as HTMLElement);
    expect(onGoTo).toHaveBeenCalledWith('/ce/weaponTagClass');
  });

  it('writes the chosen ammo set and a typed answer into the draft', async () => {
    const setup = await show('ce-on');
    fireEvent.change(screen.getByRole('combobox', { name: /Which caliber/ }), {
      target: { value: 'AmmoSet_280British' },
    });
    expect(setup.editor.draft.value?.spec.ce?.ammoSet).toBe('AmmoSet_280British');
    const spread = screen.getByRole('spinbutton', { name: /CE shot spread/ });
    fireEvent.input(spread, { target: { value: '0.2' } });
    fireEvent.blur(spread);
    expect(setup.editor.draft.value?.spec.ce?.shotSpread).toEqual({
      value: 0.2,
      source: 'answered',
    });
  });

  it('says so once every question is answered', async () => {
    const setup = await show('ce-answered');
    fireEvent.click(screen.getByRole('radio', { name: 'All derived' }));
    await setup.flush();
    expect(screen.getByText('Every Combat Extended question is answered.')).toBeTruthy();
    expect(
      screen.getByText(/a separate patch file is added in Compat\/CombatExtended\/Patches/),
    ).toBeTruthy();
  });

  it('shows the loading state before the suggestion arrives', () => {
    const setup = setupOutput();
    setup.editor.open(outputEntry('ce-on'));
    renderWithProviders(<Harness setup={setup} />);
    expect(screen.getByText('Loading Combat Extended suggestions')).toBeTruthy();
  });
});
