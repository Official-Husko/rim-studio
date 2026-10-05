import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { RANGED_FIELDS } from '../../model/fields';
import { makeEnv, WithEnv } from './fieldEnvTestkit';
import { NumberFieldRow } from './NumberFieldRow';
import { newDraft } from '../../model/draft';

const damage = RANGED_FIELDS[0];
if (!damage) throw new Error('missing field');

function specWith(source: 'typed' | 'anchor' | 'suggested') {
  const spec = newDraft('ranged', 'TM_Gun', 'gun').spec;
  return { ...spec, ranged: { accuracy: {}, damage: { value: 18, source } } };
}

describe('NumberFieldRow', () => {
  it('shows the value, the unit and the source chip', () => {
    const env = makeEnv({ spec: specWith('anchor') });
    renderWithProviders(
      <WithEnv env={env}>
        <NumberFieldRow def={damage} />
      </WithEnv>,
    );
    expect((screen.getByRole('spinbutton', { name: /Damage/ }) as HTMLInputElement).value).toBe(
      '18',
    );
    expect(screen.getByText('damage')).toBeTruthy();
    expect(screen.getByText('Anchor')).toBeTruthy();
  });

  it('writes a typed value when the user types', () => {
    const env = makeEnv({ spec: specWith('anchor') });
    renderWithProviders(
      <WithEnv env={env}>
        <NumberFieldRow def={damage} />
      </WithEnv>,
    );
    fireEvent.input(screen.getByRole('spinbutton', { name: /Damage/ }), {
      target: { value: '22' },
    });
    expect(env.setField).toHaveBeenCalledWith('/ranged/damage', { value: 22, source: 'typed' });
  });

  it('offers a suggestion and writes it only on request', () => {
    const env = makeEnv({
      suggestions: [
        {
          field: '/ranged/damage',
          stat: 'damage',
          value: 12,
          level: 'tier',
          predictor: 'median',
          n: 12,
          locked: false,
        },
      ],
      pools: [{ stat: 'damage', n: 20, min: 5, p10: 6, median: 12, p90: 18.7, max: 25 }],
    });
    renderWithProviders(
      <WithEnv env={env}>
        <NumberFieldRow def={damage} />
      </WithEnv>,
    );
    expect(screen.getByText(/p10 6, median 12, p90 18.7/)).toBeTruthy();
    expect(env.setField).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Use 12' }));
    expect(env.setField).toHaveBeenCalledWith('/ranged/damage', { value: 12, source: 'suggested' });
  });

  it('offers nothing for a locked typed value', () => {
    const env = makeEnv({
      spec: specWith('typed'),
      suggestions: [
        {
          field: '/ranged/damage',
          stat: 'damage',
          value: 22,
          source: 'typed',
          level: 'tier',
          predictor: 'median',
          n: 12,
          locked: true,
        },
      ],
    });
    renderWithProviders(
      <WithEnv env={env}>
        <NumberFieldRow def={damage} />
      </WithEnv>,
    );
    expect(screen.queryByRole('button', { name: /Use/ })).toBeNull();
  });

  it('shows an error of the field and the notes of its diagnostics', () => {
    const env = makeEnv({
      diagnostics: [
        {
          code: 'design.required-missing',
          severity: 'error',
          message: 'damage is required',
          field: '/ranged/damage',
        },
        {
          code: 'design.stat-out-of-band',
          severity: 'warning',
          message: 'unusual damage',
          field: '/ranged/damage',
        },
      ],
    });
    renderWithProviders(
      <WithEnv env={env}>
        <NumberFieldRow def={damage} />
      </WithEnv>,
    );
    expect(screen.getByRole('alert').textContent).toBe('damage is required');
    expect(screen.getByText(/unusual damage/)).toBeTruthy();
  });
});
