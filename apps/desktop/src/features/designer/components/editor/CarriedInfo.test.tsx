import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { recordedSpec } from '../../testSupport';
import { CarriedInfo } from './CarriedInfo';
import { makeEnv, WithEnv } from './fieldEnvTestkit';

describe('CarriedInfo', () => {
  it('lists the required fields the real plasma sword does not set either', () => {
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: recordedSpec('plasma-sword') })}>
        <CarriedInfo />
      </WithEnv>,
    );
    expect(screen.getByText(/does not set these required fields either/).textContent).toContain(
      '/stuff, /workToMake',
    );
  });

  it('lists the lists that are written fresh', () => {
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: recordedSpec('beam-repeater') })}>
        <CarriedInfo />
      </WithEnv>,
    );
    expect(screen.getByText('Lists written fresh')).toBeTruthy();
    expect(screen.getByText('recipeUsers')).toBeTruthy();
  });

  it('shows nothing for a weapon that carries none of it', () => {
    const { container } = renderWithProviders(
      <WithEnv env={makeEnv({ spec: recordedSpec('longsword') })}>
        <CarriedInfo />
      </WithEnv>,
    );
    expect(container.textContent).toBe('');
  });
});
