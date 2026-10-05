import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { LinkStatusLine } from './LinkStatusLine';
import { statusFixture } from './testSupport';

describe('LinkStatusLine', () => {
  it('shows the state and the sentence', () => {
    renderWithProviders(<LinkStatusLine status={statusFixture('link-status-linked')} />);
    expect(screen.getByText('Linked into the game')).toBeTruthy();
    expect(screen.getByText(/points at this project/)).toBeTruthy();
  });

  it('shows a taken name as a warning with the entry named', () => {
    renderWithProviders(
      <LinkStatusLine
        status={statusFixture('link-status-not-linked', { state: 'foreign-folder' })}
      />,
    );
    expect(screen.getByText('Name taken by a folder')).toBeTruthy();
    expect(screen.getByText(/A folder named RS_Arms already exists/)).toBeTruthy();
  });
});
