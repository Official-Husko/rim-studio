import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { recordedSpec } from '../../testSupport';
import { ExtraDamageRows } from './ExtraDamageRows';
import { makeEnv, WithEnv } from './fieldEnvTestkit';

const rows = (
  <ExtraDamageRows
    pointer="/tools/1/extraMeleeDamages"
    label="Extra damages"
    addLabel="Add extra damage"
  />
);

describe('ExtraDamageRows', () => {
  const spec = recordedSpec('plasma-sword');

  it('shows the flame damage of the real plasma sword', () => {
    renderWithProviders(<WithEnv env={makeEnv({ spec })}>{rows}</WithEnv>);
    expect((screen.getByLabelText('Damage def') as HTMLInputElement).value).toBe('Flame');
    expect((screen.getByLabelText('Amount') as HTMLInputElement).value).toBe('10');
    expect((screen.getByLabelText('Chance') as HTMLInputElement).value).toBe('0.5');
  });

  it('writes the amount as a plain number', () => {
    const env = makeEnv({ spec });
    renderWithProviders(<WithEnv env={env}>{rows}</WithEnv>);
    fireEvent.input(screen.getByLabelText('Amount'), { target: { value: '12' } });
    expect(env.setField).toHaveBeenCalledWith('/tools/1/extraMeleeDamages/0/amount', 12);
  });

  it('adds and removes rows', () => {
    const env = makeEnv({ spec });
    renderWithProviders(<WithEnv env={env}>{rows}</WithEnv>);
    fireEvent.click(screen.getByRole('button', { name: 'Add extra damage' }));
    expect(env.setField).toHaveBeenCalledWith('/tools/1/extraMeleeDamages/1', { def: '' });
    fireEvent.click(screen.getByRole('button', { name: 'Remove extra damage Flame' }));
    expect(env.setField).toHaveBeenCalledWith('/tools/1/extraMeleeDamages', undefined);
  });
});
