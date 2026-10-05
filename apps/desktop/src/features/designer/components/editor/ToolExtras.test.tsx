import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { recordedSpec } from '../../testSupport';
import { makeEnv, WithEnv } from './fieldEnvTestkit';
import { ToolExtras } from './ToolExtras';

describe('ToolExtras', () => {
  const spec = recordedSpec('zeushammer');

  it('shows the EMP damage and the other field of the real hammer head', () => {
    renderWithProviders(
      <WithEnv env={makeEnv({ spec })}>
        <ToolExtras index={1} />
      </WithEnv>,
    );
    expect((screen.getByLabelText('Damage def') as HTMLInputElement).value).toBe('EMP');
    expect(screen.getByRole('region', { name: 'XML of labelUsedInLogging' })).toBeTruthy();
  });

  it('turns the surprise attack on and off', () => {
    const env = makeEnv({ spec });
    renderWithProviders(
      <WithEnv env={env}>
        <ToolExtras index={0} />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('switch', { name: 'Surprise attack' }));
    expect(env.setField).toHaveBeenCalledWith('/tools/0/surpriseAttack', {});
  });

  it('shows the damages of an existing surprise attack', () => {
    const tools = [
      {
        ...(spec.tools?.[0] ?? { label: 'a', capacities: [] }),
        surpriseAttack: { extraMeleeDamages: [{ def: 'Stun', amount: 3 }] },
      },
    ];
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: { ...spec, tools } })}>
        <ToolExtras index={0} />
      </WithEnv>,
    );
    expect(screen.getByText('Extra damages of the surprise attack')).toBeTruthy();
    expect((screen.getByDisplayValue('Stun') as HTMLInputElement).value).toBe('Stun');
  });
});
