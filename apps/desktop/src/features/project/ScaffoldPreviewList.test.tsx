import { screen, within } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { ScaffoldPreviewList } from './ScaffoldPreviewList';
import { emptyForm, scaffoldPreview } from './scaffold';

describe('ScaffoldPreviewList', () => {
  it('lists the entries under the folder that will be created', () => {
    const entries = scaffoldPreview(emptyForm());
    renderWithProviders(<ScaffoldPreviewList root="/home/user/mods/Arsenal" entries={entries} />);
    expect(screen.getByText('/home/user/mods/Arsenal')).toBeTruthy();
    const list = screen.getByRole('list', { name: 'Files and folders that will be created' });
    expect(within(list).getByText('About.xml')).toBeTruthy();
    expect(within(list).getAllByRole('listitem')).toHaveLength(entries.length);
    expect(screen.getByText(`${entries.length} entries will be created`)).toBeTruthy();
  });

  it('uses the singular for one entry', () => {
    renderWithProviders(
      <ScaffoldPreviewList root="/x" entries={[{ path: 'About', kind: 'folder' }]} />,
    );
    expect(screen.getByText('1 entry will be created')).toBeTruthy();
  });
});
