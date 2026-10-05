import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { DraftDto } from 'rimstudio-ipc-types';
import { fixture, recordedSpec } from '../../testSupport';
import { makeEnv, WithEnv } from './fieldEnvTestkit';
import { ProjectilePanel } from './ProjectilePanel';

describe('ProjectilePanel', () => {
  it('shows the shared projectile and says damage edits do not reach the written file', () => {
    const spec = fixture<DraftDto>('designer-draft-clone-edited').spec;
    renderWithProviders(
      <WithEnv env={makeEnv({ spec })}>
        <ProjectilePanel />
      </WithEnv>,
    );
    expect((screen.getByLabelText('Projectile def') as HTMLInputElement).value).toBe(
      'Bullet_BoltActionRifle',
    );
    expect(
      screen.getByRole('switch', { name: 'Own projectile' }).getAttribute('aria-checked'),
    ).toBe('false');
    expect(screen.getByText(/only changes the numbers shown on this page/)).toBeTruthy();
    expect(screen.getByText(/Switch to an own projectile/)).toBeTruthy();
  });

  it('asks the backend for an own projectile when the switch is turned on', () => {
    const spec = fixture<DraftDto>('designer-draft-clone-edited').spec;
    const env = makeEnv({ spec });
    renderWithProviders(
      <WithEnv env={env}>
        <ProjectilePanel />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('switch', { name: 'Own projectile' }));
    expect(env.setOwnProjectile).toHaveBeenCalledWith(true);
  });

  it('turns the own projectile off through the backend too', () => {
    const spec = fixture<DraftDto>('designer-fields-draft-rifle-own-edited').spec;
    const env = makeEnv({ spec });
    renderWithProviders(
      <WithEnv env={env}>
        <ProjectilePanel />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('switch', { name: 'Own projectile' }));
    expect(env.setOwnProjectile).toHaveBeenCalledWith(false);
  });

  it('shows the fields of the own projectile of a real clone and where it was copied from', () => {
    const spec = fixture<DraftDto>('designer-fields-draft-rifle-own-edited').spec;
    renderWithProviders(
      <WithEnv env={makeEnv({ spec })}>
        <ProjectilePanel />
      </WithEnv>,
    );
    expect((screen.getByLabelText(/Def name/) as HTMLInputElement).value).toContain('Bullet');
    expect((screen.getByLabelText('Damage def') as HTMLInputElement).value).toBe('Bullet');
    expect(screen.getByText(/Copied from the projectile Bullet_BoltActionRifle/)).toBeTruthy();
    expect(screen.getByText(/editing the damage changes the written file/)).toBeTruthy();
  });

  it('shows the carried fields of the own projectile of the beam repeater', () => {
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: recordedSpec('beam-repeater') })}>
        <ProjectilePanel />
      </WithEnv>,
    );
    expect(screen.getByRole('region', { name: 'XML of beamMoteDef' })).toBeTruthy();
    expect(screen.getByRole('region', { name: 'XML of thingClass' })).toBeTruthy();
  });

  it('shows the notes of the last change and disables the switch while it runs', () => {
    const spec = fixture<DraftDto>('designer-draft-clone-edited').spec;
    renderWithProviders(
      <WithEnv env={makeEnv({ spec, ownBusy: true, projectileNotes: ['A note from the backend'] })}>
        <ProjectilePanel />
      </WithEnv>,
    );
    expect(screen.getByText('A note from the backend')).toBeTruthy();
    expect(
      (screen.getByRole('switch', { name: 'Own projectile' }) as HTMLButtonElement).disabled,
    ).toBe(true);
  });
});
