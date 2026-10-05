import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { recordedSpec } from '../../testSupport';
import { ChipListField } from './ChipListField';
import { makeEnv, WithEnv } from './fieldEnvTestkit';

describe('ChipListField', () => {
  const spec = recordedSpec('longsword');

  it('shows the entries of a real weapon as chips', () => {
    renderWithProviders(
      <WithEnv env={makeEnv({ spec })}>
        <ChipListField pointer="/weaponTags" label="Weapon tags" />
      </WithEnv>,
    );
    for (const tag of spec.weaponTags ?? []) expect(screen.getByText(tag)).toBeTruthy();
  });

  it('adds several names typed with commas and skips the ones it has', () => {
    const env = makeEnv({ spec });
    const have = spec.weaponTags ?? [];
    renderWithProviders(
      <WithEnv env={env}>
        <ChipListField pointer="/weaponTags" label="Weapon tags" />
      </WithEnv>,
    );
    fireEvent.input(screen.getByLabelText('Weapon tags'), {
      target: { value: `${have[0] ?? ''}, Extra, Other` },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add' }));
    expect(env.setField).toHaveBeenCalledWith('/weaponTags', [...have, 'Extra', 'Other']);
  });

  it('adds with Enter', () => {
    const env = makeEnv({ spec });
    renderWithProviders(
      <WithEnv env={env}>
        <ChipListField pointer="/weaponClasses" label="Weapon classes" />
      </WithEnv>,
    );
    const input = screen.getByLabelText('Weapon classes');
    fireEvent.input(input, { target: { value: 'Neolithic' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(env.setField).toHaveBeenCalledWith('/weaponClasses', [
      ...(spec.weaponClasses ?? []),
      'Neolithic',
    ]);
  });

  it('removes an entry, and removes the value when the list gets empty', () => {
    const base = makeEnv();
    const env = { ...base, spec: { ...base.spec, tradeTags: ['WeaponMelee'] } };
    renderWithProviders(
      <WithEnv env={env}>
        <ChipListField pointer="/tradeTags" label="Trade tags" />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Remove WeaponMelee' }));
    expect(env.setField).toHaveBeenCalledWith('/tradeTags', undefined);
  });

  it('keeps an empty list when the contract needs the list', () => {
    const base = makeEnv();
    const env = { ...base, spec: { ...base.spec, tradeTags: ['A'] } };
    renderWithProviders(
      <WithEnv env={env}>
        <ChipListField pointer="/tradeTags" label="Trade tags" keepEmpty />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Remove A' }));
    expect(env.setField).toHaveBeenCalledWith('/tradeTags', []);
  });
});
