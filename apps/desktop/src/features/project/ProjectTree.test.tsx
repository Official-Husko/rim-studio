import { fireEvent, screen, within } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { ProjectTreeDto } from 'rimstudio-ipc-types';
import { ProjectTree } from './ProjectTree';

const gewehr = loadFixture<ProjectTreeDto>('project-tree-gewehr');
const lone = loadFixture<ProjectTreeDto>('project-tree-lonewolf');

describe('ProjectTree', () => {
  it('draws roles, counts and sizes on the folders', () => {
    renderWithProviders(<ProjectTree tree={gewehr} selectedPath={undefined} onSelect={vi.fn()} />);
    const root = screen.getByRole('treeitem', { name: /\[OH\] Gewehr 41/ });
    expect(within(root).getByText('Content')).toBeTruthy();
    expect(within(root).getByText('27 / 10.9 MiB')).toBeTruthy();
    expect(within(root).getByText('3 issues')).toBeTruthy();
    const source = screen.getByRole('treeitem', { name: /^Source/ });
    expect(within(source).getByText('6 / 9.1 MiB')).toBeTruthy();
  });

  it('marks the issue of a file and shows its size', () => {
    renderWithProviders(<ProjectTree tree={gewehr} selectedPath={undefined} onSelect={vi.fn()} />);
    const file = screen.getByRole('treeitem', { name: /ce_patch\.xml/ });
    expect(within(file).getByText('1 issue')).toBeTruthy();
    expect(within(file).getByText('8.9 KiB')).toBeTruthy();
  });

  it('reports the node that is chosen', () => {
    const onSelect = vi.fn();
    renderWithProviders(<ProjectTree tree={gewehr} selectedPath={undefined} onSelect={onSelect} />);
    fireEvent.click(screen.getByRole('treeitem', { name: /About\.xml/ }));
    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ path: 'About/About.xml' }));
  });

  it('opens the folders above the selected file', () => {
    renderWithProviders(
      <ProjectTree
        tree={lone}
        selectedPath="Common/Defs/ThingsDef_Misc/Weapons/Weapons_Ranged.xml"
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByRole('treeitem', { name: /Weapons_Ranged\.xml/ })).toBeTruthy();
  });

  it('says when the tree is cut short', () => {
    renderWithProviders(
      <ProjectTree
        tree={{ ...gewehr, truncated: true }}
        selectedPath={undefined}
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByText(/The tree is cut short/)).toBeTruthy();
  });
});
