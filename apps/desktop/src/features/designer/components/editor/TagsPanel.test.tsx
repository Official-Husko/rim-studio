import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { DraftDto } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import { makeEnv, WithEnv } from './fieldEnvTestkit';
import { TagsPanel } from './TagsPanel';

describe('TagsPanel', () => {
  it('shows the tags, trade tags and classes of the clone as chips', () => {
    const spec = fixture<DraftDto>('designer-draft-clone-edited').spec;
    renderWithProviders(
      <WithEnv env={makeEnv({ spec })}>
        <TagsPanel />
      </WithEnv>,
    );
    expect(screen.getByText('Gun')).toBeTruthy();
    expect(screen.getByText('Ranged')).toBeTruthy();
    expect(screen.getByText('LongShots')).toBeTruthy();
    expect(screen.getByText('WeaponRanged')).toBeTruthy();
  });
});
