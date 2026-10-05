import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { ConvertView } from './ConvertView';
import { runScan } from './scanStore';
import { installTransport, openFixtureProject } from './testSupport';

describe('ConvertView', () => {
  it('shows the scan table next to the focused weapon', async () => {
    installTransport();
    const p = openFixtureProject();
    const project = { projectId: p.projectId, path: p.path, name: p.name };
    render(<ConvertView project={project} />);
    expect(screen.getByLabelText(/Scanning the weapons of the mod/)).toBeTruthy();
    await runScan(project);
    expect(await screen.findByRole('grid', { name: 'Weapons of the mod' })).toBeTruthy();
    expect(screen.getByRole('separator', { name: 'Resize the weapon list' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Gewehr 41 (m)' })).toBeTruthy();
  });
});
