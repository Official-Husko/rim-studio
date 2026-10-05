import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { beforeEach, describe, expect, it } from 'vitest';
import { currentProject, recentProjects, setCurrentProject } from '~/shared/project';
import { RecentList } from './RecentList';
import { installTransport } from './testSupport';

beforeEach(() => {
  installTransport();
});

describe('RecentList', () => {
  it('says so when there are no recent projects', () => {
    renderWithProviders(<RecentList />);
    expect(screen.getByText('No recent projects')).toBeTruthy();
  });

  it('lists the recent projects newest first', () => {
    setCurrentProject({ projectId: 'p-1', path: '/m/One', name: 'One', packageId: 'a.one' });
    setCurrentProject({ projectId: 'p-2', path: '/m/Two', name: 'Two' });
    renderWithProviders(<RecentList />);
    const items = screen.getAllByRole('listitem');
    expect(items).toHaveLength(2);
    expect(items[0]?.textContent).toContain('Two');
    expect(items[1]?.textContent).toContain('a.one');
  });

  it('opens a project from the list', async () => {
    setCurrentProject({ projectId: 'p-1', path: '/m/One', name: 'One' });
    setCurrentProject(undefined);
    renderWithProviders(<RecentList />);
    fireEvent.click(screen.getByRole('button', { name: /^One/ }));
    await screen.findByRole('list', { name: 'Recent projects' });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(currentProject.value).toBeDefined();
  });

  it('forgets a project without touching anything else', () => {
    setCurrentProject({ projectId: 'p-1', path: '/m/One', name: 'One' });
    renderWithProviders(<RecentList />);
    fireEvent.click(screen.getByRole('button', { name: 'Remove One from the list' }));
    expect(recentProjects.value).toEqual([]);
  });
});
