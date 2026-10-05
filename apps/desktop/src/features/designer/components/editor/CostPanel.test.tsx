import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { DraftDto } from 'rimstudio-ipc-types';
import { fixture, recordedSpec } from '../../testSupport';
import { CostPanel } from './CostPanel';
import { makeEnv, WithEnv } from './fieldEnvTestkit';

describe('CostPanel', () => {
  const spec = fixture<DraftDto>('designer-draft-clone-edited').spec;

  it('shows mass, work and the real ingredients', () => {
    renderWithProviders(
      <WithEnv env={makeEnv({ spec })}>
        <CostPanel kind="ranged" />
      </WithEnv>,
    );
    expect((screen.getByRole('spinbutton', { name: /Mass/ }) as HTMLInputElement).value).toBe(
      '3.5',
    );
    expect((screen.getAllByLabelText('Ingredient def')[0] as HTMLInputElement).value).toBe('Steel');
    expect(
      (screen.getAllByRole('spinbutton', { name: /Count/ })[0] as HTMLInputElement).value,
    ).toBe('60');
  });

  it('adds and removes an ingredient', () => {
    const env = makeEnv({ spec });
    renderWithProviders(
      <WithEnv env={env}>
        <CostPanel kind="ranged" />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Add ingredient' }));
    expect(env.setField).toHaveBeenCalledWith('/costList/2', { defName: '', count: 1 });
    fireEvent.click(screen.getByRole('button', { name: 'Remove ingredient Steel' }));
    expect(env.setField).toHaveBeenCalledWith('/costList/0', undefined);
  });

  it('writes a count as a plain number', () => {
    const env = makeEnv({ spec });
    renderWithProviders(
      <WithEnv env={env}>
        <CostPanel kind="ranged" />
      </WithEnv>,
    );
    fireEvent.input(screen.getAllByRole('spinbutton', { name: /Count/ })[0] as HTMLElement, {
      target: { value: '70' },
    });
    expect(env.setField).toHaveBeenCalledWith('/costList/0/count', 70);
  });

  it('shows the stuff fields only for melee', () => {
    const { rerender } = renderWithProviders(
      <WithEnv env={makeEnv({ spec })}>
        <CostPanel kind="ranged" />
      </WithEnv>,
    );
    expect(screen.queryByLabelText('Stuff categories')).toBeNull();
    rerender(
      <WithEnv env={makeEnv({ spec })}>
        <CostPanel kind="melee" />
      </WithEnv>,
    );
    expect(screen.getByLabelText('Stuff categories')).toBeTruthy();
  });

  it('shows the recipe of the real longsword with its skill requirement', () => {
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: recordedSpec('longsword') })}>
        <CostPanel kind="melee" />
      </WithEnv>,
    );
    expect((screen.getByLabelText('Skill 1') as HTMLInputElement).value).toBe('Crafting');
    expect((screen.getByLabelText('Level 1') as HTMLInputElement).value).toBe('5');
  });
});
