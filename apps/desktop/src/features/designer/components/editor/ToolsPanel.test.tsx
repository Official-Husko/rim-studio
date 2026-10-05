import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { DraftDto } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import { makeEnv, WithEnv } from './fieldEnvTestkit';
import { ToolsPanel } from './ToolsPanel';

describe('ToolsPanel', () => {
  it('lists the tools of a melee weapon with their numbers', () => {
    const spec = fixture<DraftDto>('designer-draft-melee').spec;
    renderWithProviders(
      <WithEnv env={makeEnv({ spec })}>
        <ToolsPanel kind="melee" />
      </WithEnv>,
    );
    expect((screen.getByLabelText('Tool label') as HTMLInputElement).value).toBe('blade');
    expect(screen.getByText('Cut')).toBeTruthy();
    expect((screen.getByRole('spinbutton', { name: /Damage/ }) as HTMLInputElement).value).toBe(
      '18',
    );
  });

  it('adds and removes a tool', () => {
    const spec = fixture<DraftDto>('designer-draft-melee').spec;
    const env = makeEnv({ spec });
    renderWithProviders(
      <WithEnv env={env}>
        <ToolsPanel kind="melee" />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Add tool' }));
    expect(env.setField).toHaveBeenCalledWith('/tools/1', { label: '', capacities: [] });
    fireEvent.click(screen.getByRole('button', { name: 'Remove tool blade' }));
    expect(env.setField).toHaveBeenCalledWith('/tools/0', undefined);
  });

  it('says a melee weapon needs a tool when there is none', () => {
    renderWithProviders(
      <WithEnv env={makeEnv()}>
        <ToolsPanel kind="melee" />
      </WithEnv>,
    );
    expect(screen.getByText(/needs at least one/)).toBeTruthy();
  });
});
