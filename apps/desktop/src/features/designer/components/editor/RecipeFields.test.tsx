import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { recordedSpec } from '../../testSupport';
import { makeEnv, WithEnv } from './fieldEnvTestkit';
import { RecipeFields } from './RecipeFields';

describe('RecipeFields', () => {
  it('shows the real recipe of the beam repeater', () => {
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: recordedSpec('beam-repeater') })}>
        <RecipeFields />
      </WithEnv>,
    );
    expect((screen.getByLabelText('Skill 1') as HTMLInputElement).value).toBe('Crafting');
    expect((screen.getByLabelText('Level 1') as HTMLInputElement).value).toBe('8');
    expect(
      (screen.getByRole('spinbutton', { name: 'Display priority' }) as HTMLInputElement).value,
    ).toBe('220');
    expect(screen.getByText('FabricationBench')).toBeTruthy();
  });

  it('offers to add recipe settings when the weapon has none', () => {
    const env = makeEnv({ spec: recordedSpec('plasma-sword') });
    renderWithProviders(
      <WithEnv env={env}>
        <RecipeFields />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Add recipe settings' }));
    expect(env.setField).toHaveBeenCalledWith('/recipe', {});
  });

  it('removes the recipe settings', () => {
    const env = makeEnv({ spec: recordedSpec('longsword') });
    renderWithProviders(
      <WithEnv env={env}>
        <RecipeFields />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Remove recipe settings' }));
    expect(env.setField).toHaveBeenCalledWith('/recipe', undefined);
  });

  it('writes the work skill typed by the user', () => {
    const env = makeEnv({ spec: recordedSpec('longsword') });
    renderWithProviders(
      <WithEnv env={env}>
        <RecipeFields />
      </WithEnv>,
    );
    fireEvent.input(screen.getByLabelText('Work skill'), { target: { value: 'Crafting' } });
    expect(env.setField).toHaveBeenCalledWith('/recipe/workSkill', 'Crafting');
  });
});
