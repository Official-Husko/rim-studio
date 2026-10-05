import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { DraftDto } from 'rimstudio-ipc-types';
import { fixture, recordedSpec } from '../../testSupport';
import { makeEnv, WithEnv } from './fieldEnvTestkit';
import { OtherFieldsPanel } from './OtherFieldsPanel';

describe('OtherFieldsPanel', () => {
  it('shows the stats inherited from the parent', () => {
    const spec = fixture<DraftDto>('designer-draft-clone-edited').spec;
    renderWithProviders(
      <WithEnv env={makeEnv({ spec })}>
        <OtherFieldsPanel />
      </WithEnv>,
    );
    expect(screen.getByText('MaxHitPoints')).toBeTruthy();
    expect(screen.getByText('100')).toBeTruthy();
  });

  it('shows the carried fields of a real clone as XML and the offsets of the minigun', () => {
    const spec = recordedSpec('beam-repeater');
    renderWithProviders(
      <WithEnv env={makeEnv({ spec })}>
        <OtherFieldsPanel />
      </WithEnv>,
    );
    expect(screen.getByRole('region', { name: 'XML of aimingChargeMote' })).toBeTruthy();
    expect((screen.getByLabelText('Stat 1') as HTMLInputElement).value).toBe('MoveSpeed');
    expect((screen.getByLabelText('Offset 1') as HTMLInputElement).value).toBe('-0.25');
  });

  it('shows the other definition fields of the longsword', () => {
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: recordedSpec('longsword') })}>
        <OtherFieldsPanel />
      </WithEnv>,
    );
    expect(
      screen.getByRole('region', { name: 'XML of equippedAngleOffset' }).textContent,
    ).toContain('-65');
  });

  it('writes the forced miss radius of a gun as a plain number', () => {
    const env = makeEnv({ spec: recordedSpec('beam-repeater') });
    renderWithProviders(
      <WithEnv env={env}>
        <OtherFieldsPanel />
      </WithEnv>,
    );
    fireEvent.input(screen.getByRole('spinbutton', { name: /Forced miss radius/ }), {
      target: { value: '1.5' },
    });
    expect(env.setField).toHaveBeenCalledWith('/ranged/forcedMissRadius', 1.5);
  });
});
