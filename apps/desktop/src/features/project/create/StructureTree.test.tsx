import { screen, within } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { StructureTree } from './StructureTree';

describe('StructureTree', () => {
  it('lists folders and files indented with what each is for', () => {
    renderWithProviders(
      <StructureTree
        root="/mods/New"
        entries={[
          { path: 'About', kind: 'folder' },
          { path: 'About/About.xml', kind: 'file' },
          { path: '1.6', kind: 'folder' },
          { path: 'Mystery', kind: 'folder' },
        ]}
      />,
    );
    const list = screen.getByRole('list', { name: 'Structure of the new mod' });
    expect(within(list).getAllByRole('listitem')).toHaveLength(4);
    expect(within(list).getByText('About.xml')).toBeTruthy();
    expect(within(list).getByText('Content that loads only on game version 1.6.')).toBeTruthy();
    expect(screen.getByText('/mods/New')).toBeTruthy();
  });
});
