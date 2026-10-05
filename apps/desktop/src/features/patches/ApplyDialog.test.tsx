import { fireEvent, render, screen, waitFor, within } from '@testing-library/preact';
import { loadFixture } from 'rimstudio-testkit';
import type { ConvertScanDto, WritePlanDto } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import { ApplyDialog } from './ApplyDialog';
import { applyState, closeReview, openReview } from './applyStore';
import { installTransport, openFixtureProject } from './testSupport';

const candidates = loadFixture<ConvertScanDto>('designer_convert_scan').candidates;

describe('ApplyDialog', () => {
  it('renders nothing while closed', () => {
    installTransport();
    const p = openFixtureProject();
    render(<ApplyDialog project={{ projectId: p.projectId, path: p.path, name: p.name }} />);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('passes the backup and dry apply choices to the apply call', async () => {
    const transport = installTransport({
      designer_export_plan: () => loadFixture<WritePlanDto>('patches-plan-ready'),
    });
    const p = openFixtureProject();
    const project = { projectId: p.projectId, path: p.path, name: p.name };
    render(<ApplyDialog project={project} />);
    await openReview(project, candidates.slice(0, 1));
    const dialog = await screen.findByRole('dialog', { name: 'Apply the conversion' });
    fireEvent.click(
      within(dialog).getByRole('switch', { name: 'Back up files that are replaced' }),
    );
    fireEvent.click(
      within(dialog).getByRole('switch', {
        name: 'Test the patch on a scratch copy after writing',
      }),
    );
    fireEvent.click(within(dialog).getByRole('button', { name: 'Write 1 weapon' }));
    await waitFor(() => expect(applyState.value.phase).toBe('done'));
    expect(transport.calls.find((c) => c.name === 'designer_apply_plan')?.request).toMatchObject({
      backup: false,
      dryApply: false,
    });
    closeReview();
  });
});
