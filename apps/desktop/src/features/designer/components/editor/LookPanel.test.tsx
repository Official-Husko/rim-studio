import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { recordedSpec } from '../../testSupport';
import { makeEnv, WithEnv } from './fieldEnvTestkit';
import { LookPanel } from './LookPanel';

describe('LookPanel', () => {
  it('shows the texture, the sound and the icon of a real weapon', () => {
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: recordedSpec('plasma-sword') })}>
        <LookPanel />
      </WithEnv>,
    );
    expect((screen.getByLabelText('Texture path') as HTMLInputElement).value).toContain(
      'PlasmaSword',
    );
    expect((screen.getByLabelText('Interact sound') as HTMLInputElement).value).toBe(
      'Interact_PlasmaSword',
    );
  });

  it('writes the icon scale as a plain number', () => {
    const env = makeEnv({ spec: recordedSpec('longsword') });
    renderWithProviders(
      <WithEnv env={env}>
        <LookPanel />
      </WithEnv>,
    );
    fireEvent.input(screen.getByRole('spinbutton', { name: 'Icon scale' }), {
      target: { value: '0.8' },
    });
    expect(env.setField).toHaveBeenCalledWith('/uiIconScale', 0.8);
  });
});
