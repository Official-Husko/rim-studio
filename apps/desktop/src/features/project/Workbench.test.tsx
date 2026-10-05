import { fireEvent, screen, within } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { beforeEach, describe, expect, it } from 'vitest';
import type { ProjectLayoutCheckDto, ProjectSummaryDto, ProjectTreeDto } from 'rimstudio-ipc-types';
import { Workbench } from './Workbench';
import { loadProject, view } from './store';
import { gewehrRef, installTransport } from './testSupport';

beforeEach(async () => {
  installTransport();
  await loadProject(gewehrRef());
});

function current() {
  const shown = view.value;
  if (!shown) throw new Error('project not loaded');
  return shown;
}

describe('Workbench', () => {
  it('starts with nothing chosen', () => {
    renderWithProviders(<Workbench view={current()} />);
    expect(screen.getByText('No file selected')).toBeTruthy();
    expect(screen.getByRole('tree', { name: 'Project folders' })).toBeTruthy();
  });

  it('shows what a folder holds when it is chosen', () => {
    renderWithProviders(<Workbench view={current()} />);
    const tree = screen.getByRole('tree', { name: 'Project folders' });
    fireEvent.click(within(tree).getByRole('treeitem', { name: /Patches/ }));
    expect(screen.getByText('1 layout issue on or below this folder')).toBeTruthy();
  });

  it('opens a file in the viewer', async () => {
    renderWithProviders(<Workbench view={current()} />);
    const tree = screen.getByRole('tree', { name: 'Project folders' });
    fireEvent.click(within(tree).getByRole('treeitem', { name: /About\.xml/ }));
    expect(await screen.findByRole('region', { name: 'Contents of About/About.xml' })).toBeTruthy();
  });

  it('works for a project without issues', () => {
    renderWithProviders(
      <Workbench
        view={{
          summary: loadFixture<ProjectSummaryDto>('project-create-new'),
          tree: loadFixture<ProjectTreeDto>('project-tree-new'),
          check: loadFixture<ProjectLayoutCheckDto>('layout-check-new'),
        }}
      />,
    );
    expect(screen.getByRole('tree', { name: 'Project folders' })).toBeTruthy();
  });
});
