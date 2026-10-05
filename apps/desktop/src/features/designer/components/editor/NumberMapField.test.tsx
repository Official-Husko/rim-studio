import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { recordedSpec } from '../../testSupport';
import { makeEnv, WithEnv } from './fieldEnvTestkit';
import { NumberMapField } from './NumberMapField';

function field() {
  return (
    <NumberMapField
      pointer="/recipe/skillRequirements"
      label="Skill requirements"
      keyLabel="Skill"
      valueLabel="Level"
      addLabel="Add skill"
      removeLabel={(name) => `Remove skill ${name}`}
    />
  );
}

describe('NumberMapField', () => {
  const spec = recordedSpec('longsword');

  it('shows the real skill requirements of the longsword as rows', () => {
    renderWithProviders(<WithEnv env={makeEnv({ spec })}>{field()}</WithEnv>);
    expect((screen.getByLabelText('Skill 1') as HTMLInputElement).value).toBe('Crafting');
    expect((screen.getByLabelText('Level 1') as HTMLInputElement).value).toBe('5');
  });

  it('writes the whole map when a level changes', () => {
    const env = makeEnv({ spec });
    renderWithProviders(<WithEnv env={env}>{field()}</WithEnv>);
    fireEvent.input(screen.getByLabelText('Level 1'), { target: { value: '8' } });
    expect(env.setField).toHaveBeenCalledWith('/recipe/skillRequirements', { Crafting: 8 });
  });

  it('keeps a new row on screen but writes it only when it has a skill and a level', () => {
    const env = makeEnv({ spec });
    renderWithProviders(<WithEnv env={env}>{field()}</WithEnv>);
    fireEvent.click(screen.getByRole('button', { name: 'Add skill' }));
    expect(screen.getByLabelText('Skill 2')).toBeTruthy();
    fireEvent.input(screen.getByLabelText('Skill 2'), { target: { value: 'Shooting' } });
    expect(env.setField).toHaveBeenLastCalledWith('/recipe/skillRequirements', { Crafting: 5 });
    fireEvent.input(screen.getByLabelText('Level 2'), { target: { value: '3' } });
    expect(env.setField).toHaveBeenLastCalledWith('/recipe/skillRequirements', {
      Crafting: 5,
      Shooting: 3,
    });
  });

  it('removes a row, and removes the map when it is empty', () => {
    const env = makeEnv({ spec });
    renderWithProviders(<WithEnv env={env}>{field()}</WithEnv>);
    fireEvent.click(screen.getByRole('button', { name: 'Remove skill Crafting' }));
    expect(env.setField).toHaveBeenCalledWith('/recipe/skillRequirements', undefined);
  });

  it('follows an outside change of the map', () => {
    const env = makeEnv({ spec });
    const view = renderWithProviders(<WithEnv env={env}>{field()}</WithEnv>);
    const next = { ...env, spec: { ...spec, recipe: { skillRequirements: { Artistic: 2 } } } };
    view.rerender(<WithEnv env={next}>{field()}</WithEnv>);
    expect((screen.getByLabelText('Skill 1') as HTMLInputElement).value).toBe('Artistic');
  });
});
