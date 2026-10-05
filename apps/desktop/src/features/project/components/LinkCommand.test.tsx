import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { LinkCommand } from './LinkCommand';

describe('LinkCommand', () => {
  it('shows a shell command with a copy button', () => {
    renderWithProviders(<LinkCommand command="ln -s '/a/RS_Arms' '/g/Mods/RS_Arms'" />);
    expect(screen.getByText('Or create the link yourself')).toBeTruthy();
    expect(screen.getByText(/Run this in a terminal/)).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Copy command' })).toBeTruthy();
    expect(screen.getByText(/ln -s/)).toBeTruthy();
  });

  it('explains a Windows junction', () => {
    renderWithProviders(
      <LinkCommand command={'mklink /J "C:\\g\\Mods\\RS_Arms" "C:\\m\\RS_Arms"'} />,
    );
    expect(screen.getByText(/Command Prompt/)).toBeTruthy();
  });
});
