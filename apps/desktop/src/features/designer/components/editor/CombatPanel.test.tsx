import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { DraftDto } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import { CombatPanel } from './CombatPanel';
import { makeEnv, WithEnv } from './fieldEnvTestkit';

describe('CombatPanel', () => {
  it('shows every number of a ranged weapon with its unit', () => {
    const spec = fixture<DraftDto>('designer-draft-clone-edited').spec;
    renderWithProviders(
      <WithEnv env={makeEnv({ spec })}>
        <CombatPanel kind="ranged" />
      </WithEnv>,
    );
    expect((screen.getByRole('spinbutton', { name: /^Damage/ }) as HTMLInputElement).value).toBe(
      '22',
    );
    expect((screen.getByRole('spinbutton', { name: /Range/ }) as HTMLInputElement).value).toBe(
      '36.9',
    );
    expect((screen.getByRole('spinbutton', { name: /Warmup/ }) as HTMLInputElement).value).toBe(
      '1.7',
    );
    expect(screen.getByRole('spinbutton', { name: /Touch/ })).toBeTruthy();
    expect(screen.getByRole('spinbutton', { name: /Long/ })).toBeTruthy();
  });

  it('points a melee weapon to its tools', () => {
    renderWithProviders(
      <WithEnv env={makeEnv()}>
        <CombatPanel kind="melee" />
      </WithEnv>,
    );
    expect(screen.getByText(/Edit them under Tools/)).toBeTruthy();
  });
});
