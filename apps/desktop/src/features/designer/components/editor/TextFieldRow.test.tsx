import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { makeEnv, WithEnv } from './fieldEnvTestkit';
import { TextFieldRow } from './TextFieldRow';

describe('TextFieldRow', () => {
  it('writes a text on every input and removes it when emptied', () => {
    const env = makeEnv();
    renderWithProviders(
      <WithEnv env={env}>
        <TextFieldRow pointer="/texturePath" label="Texture path" />
      </WithEnv>,
    );
    const input = screen.getByLabelText('Texture path');
    fireEvent.input(input, { target: { value: 'Things/Gun' } });
    expect(env.setField).toHaveBeenLastCalledWith('/texturePath', 'Things/Gun');
    fireEvent.input(input, { target: { value: '' } });
    expect(env.setField).toHaveBeenLastCalledWith('/texturePath', undefined);
  });

  it('keeps an empty identity text as an empty string', () => {
    const env = makeEnv();
    renderWithProviders(
      <WithEnv env={env}>
        <TextFieldRow pointer="/identity/label" label="Label" keepEmpty />
      </WithEnv>,
    );
    fireEvent.input(screen.getByLabelText('Label'), { target: { value: '' } });
    expect(env.setField).toHaveBeenLastCalledWith('/identity/label', '');
  });

  it('commits a list on blur so a trailing comma survives typing', () => {
    const env = makeEnv();
    renderWithProviders(
      <WithEnv env={env}>
        <TextFieldRow pointer="/weaponTags" label="Weapon tags" list />
      </WithEnv>,
    );
    const input = screen.getByLabelText('Weapon tags') as HTMLInputElement;
    fireEvent.input(input, { target: { value: 'Gun, ' } });
    expect(env.setField).not.toHaveBeenCalled();
    expect(input.value).toBe('Gun, ');
    fireEvent.blur(input);
    expect(env.setField).toHaveBeenCalledWith('/weaponTags', ['Gun']);
  });

  it('shows the error of the diagnostic pointer', () => {
    const env = makeEnv({
      diagnostics: [
        {
          code: 'design.required-missing',
          severity: 'error',
          message: 'parent base is required',
          field: '/parent',
        },
      ],
    });
    renderWithProviders(
      <WithEnv env={env}>
        <TextFieldRow pointer="/parent/defName" diagnosticPointer="/parent" label="Parent base" />
      </WithEnv>,
    );
    expect(screen.getByRole('alert').textContent).toBe('parent base is required');
  });
});
