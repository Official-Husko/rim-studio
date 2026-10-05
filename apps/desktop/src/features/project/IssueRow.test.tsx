import { fireEvent, screen } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { LayoutIssueDto, ProjectLayoutCheckDto } from 'rimstudio-ipc-types';
import { IssueRow, ISSUE_TITLES } from './IssueRow';

const check = loadFixture<ProjectLayoutCheckDto>('layout-check-gewehr');
const warning = check.issues.find((i) => i.code === 'layout.ce-outside-gate') as LayoutIssueDto;
const folder = check.issues.find((i) => i.code === 'layout.missing-folder') as LayoutIssueDto;

function renderRow(issue: LayoutIssueDto, onShowPath = vi.fn()) {
  renderWithProviders(
    <ul>
      <IssueRow issue={issue} onShowPath={onShowPath} />
    </ul>,
  );
  return onShowPath;
}

describe('IssueRow', () => {
  it('shows severity, title, explanation and the suggestion of a manual fix', () => {
    renderRow(warning);
    expect(screen.getByText('Warning')).toBeTruthy();
    expect(screen.getByText('Combat Extended content is not gated')).toBeTruthy();
    expect(screen.getByText(warning.message)).toBeTruthy();
    expect(screen.getByText('Suggestion:')).toBeTruthy();
    expect(screen.queryByText('Automatic fix')).toBeNull();
  });

  it('opens the path of a file issue', () => {
    const onShowPath = renderRow(warning);
    fireEvent.click(screen.getByRole('button', { name: 'Patches/ce_patch.xml' }));
    expect(onShowPath).toHaveBeenCalledWith('Patches/ce_patch.xml');
  });

  it('marks an automatic fix and shows a missing folder as plain text', () => {
    renderRow(folder);
    expect(screen.getByText('Automatic fix')).toBeTruthy();
    expect(screen.getByText('Fix:')).toBeTruthy();
    expect(screen.queryByRole('button')).toBeNull();
  });

  it('falls back to the code for an unknown issue', () => {
    renderRow({ ...warning, code: 'layout.future-thing' });
    expect(screen.getByText('layout.future-thing')).toBeTruthy();
  });

  it('has a title for every code of the layout check', () => {
    expect(Object.keys(ISSUE_TITLES)).toHaveLength(12);
  });
});
