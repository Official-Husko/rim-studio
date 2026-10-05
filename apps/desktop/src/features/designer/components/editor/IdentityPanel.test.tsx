import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { makeEnv, WithEnv } from './fieldEnvTestkit';
import { IdentityPanel } from './IdentityPanel';

describe('IdentityPanel', () => {
  it('shows the names, tier, role and parent of the draft', () => {
    const base = makeEnv({ roles: ['pistol', 'sniper'] });
    const env = {
      ...base,
      spec: {
        ...base.spec,
        techLevel: 'industrial' as const,
        role: 'sniper',
        parent: { defName: 'BaseHumanMakeableGun' },
      },
    };
    renderWithProviders(
      <WithEnv env={env}>
        <IdentityPanel />
      </WithEnv>,
    );
    expect((screen.getByLabelText(/Def name/) as HTMLInputElement).value).toBe('TM_Gun');
    expect((screen.getByLabelText(/Tier/) as HTMLSelectElement).value).toBe('industrial');
    expect((screen.getByLabelText(/Role/) as HTMLSelectElement).value).toBe('sniper');
    expect((screen.getByLabelText(/Parent base/) as HTMLInputElement).value).toBe(
      'BaseHumanMakeableGun',
    );
  });

  it('writes the tier and role the user picks', () => {
    const env = makeEnv({ roles: ['pistol', 'sniper'] });
    renderWithProviders(
      <WithEnv env={env}>
        <IdentityPanel />
      </WithEnv>,
    );
    fireEvent.change(screen.getByLabelText(/Tier/), { target: { value: 'medieval' } });
    expect(env.setField).toHaveBeenCalledWith('/techLevel', 'medieval');
    fireEvent.change(screen.getByLabelText(/Role/), { target: { value: 'pistol' } });
    expect(env.setField).toHaveBeenCalledWith('/role', 'pistol');
  });

  it('shows the required-missing error on the tier', () => {
    const env = makeEnv({
      diagnostics: [
        {
          code: 'design.required-missing',
          severity: 'error',
          message: 'tier is required',
          field: '/techLevel',
        },
      ],
    });
    renderWithProviders(
      <WithEnv env={env}>
        <IdentityPanel />
      </WithEnv>,
    );
    expect(screen.getByText('tier is required')).toBeTruthy();
  });

  it('says the parent supplies the tier when the draft has none', () => {
    const base = makeEnv();
    const env = {
      ...base,
      spec: {
        ...base.spec,
        parent: { defName: 'BaseMeleeWeapon', inheritedTechLevel: 'medieval' as const },
      },
    };
    renderWithProviders(
      <WithEnv env={env}>
        <IdentityPanel />
      </WithEnv>,
    );
    expect(screen.getByText('The parent base supplies the tier: Medieval.')).toBeTruthy();
  });
});
