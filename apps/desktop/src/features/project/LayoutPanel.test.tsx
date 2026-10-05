import { fireEvent, screen } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { ProjectLayoutCheckDto } from 'rimstudio-ipc-types';
import { LayoutPanel } from './LayoutPanel';

const check = loadFixture<ProjectLayoutCheckDto>('layout-check-gewehr');
const base = { fixing: false, fixResult: undefined, fixError: undefined, onShowPath: vi.fn() };

describe('LayoutPanel', () => {
  it('counts the issues by severity and lists them', () => {
    renderWithProviders(<LayoutPanel check={check} onFix={vi.fn()} {...base} />);
    expect(screen.getByText('1 warning')).toBeTruthy();
    expect(screen.getByText('2 notes')).toBeTruthy();
    expect(screen.getAllByRole('listitem')).toHaveLength(3);
  });

  it('lists several findings of the same code and path without a duplicate key', () => {
    const first = check.issues[0];
    if (!first) throw new Error('fixture has no issue');
    const twice = { ...check, issues: [first, first] };
    const errors = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    renderWithProviders(<LayoutPanel check={twice} onFix={vi.fn()} {...base} />);
    expect(screen.getAllByRole('listitem')).toHaveLength(2);
    expect(errors).not.toHaveBeenCalled();
    errors.mockRestore();
  });

  it('offers the automatic fix once, for every fixable folder', () => {
    const onFix = vi.fn();
    renderWithProviders(<LayoutPanel check={check} onFix={onFix} {...base} />);
    fireEvent.click(screen.getByRole('button', { name: 'Create 2 missing folders' }));
    expect(onFix).toHaveBeenCalledTimes(1);
  });

  it('shows no fix button when nothing is fixable', () => {
    renderWithProviders(
      <LayoutPanel check={{ ...check, autoFixable: 0 }} onFix={vi.fn()} {...base} />,
    );
    expect(screen.queryByRole('button', { name: /missing folder/ })).toBeNull();
  });

  it('says that a clean project is in order', () => {
    const clean = loadFixture<ProjectLayoutCheckDto>('layout-check-new');
    renderWithProviders(<LayoutPanel check={clean} onFix={vi.fn()} {...base} />);
    expect(screen.getByText('The layout is in order')).toBeTruthy();
  });

  it('reports what the fix created and what went wrong', () => {
    const { rerender } = renderWithProviders(
      <LayoutPanel
        check={check}
        onFix={vi.fn()}
        {...base}
        fixResult={{
          projectId: 'p',
          dryRun: false,
          folders: ['Defs/SoundDefs'],
          files: [],
          skipped: [],
        }}
      />,
    );
    expect(screen.getByText('Created 1 folder')).toBeTruthy();
    rerender(
      <LayoutPanel
        check={check}
        onFix={vi.fn()}
        {...base}
        fixError={{ code: 'project.path-outside-root', message: 'refused', errorId: 'e-1' }}
      />,
    );
    expect(screen.getByText('refused')).toBeTruthy();
  });

  it('offers Fix all safe and History when the findings have fixes', () => {
    const onFixAll = vi.fn();
    const onHistory = vi.fn();
    renderWithProviders(
      <LayoutPanel
        check={check}
        onFix={vi.fn()}
        onFixAll={onFixAll}
        onHistory={onHistory}
        {...base}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Fix all safe' }));
    fireEvent.click(screen.getByRole('button', { name: 'History' }));
    expect(onFixAll).toHaveBeenCalledTimes(1);
    expect(onHistory).toHaveBeenCalledTimes(1);
  });

  it('opens the fix of one finding from its row only for findings the plan can fix', () => {
    const onFixIssue = vi.fn();
    renderWithProviders(
      <LayoutPanel check={check} onFix={vi.fn()} onFixIssue={onFixIssue} {...base} />,
    );
    const buttons = screen.getAllByRole('button', { name: /^Fix / });
    expect(buttons).toHaveLength(1);
    fireEvent.click(buttons[0] as HTMLElement);
    expect(onFixIssue).toHaveBeenCalledWith(
      expect.objectContaining({ code: 'layout.ce-outside-gate' }),
    );
  });

  it('hides Fix all safe when no finding has a planned fix', () => {
    const clean = loadFixture<ProjectLayoutCheckDto>('layout-check-new');
    renderWithProviders(<LayoutPanel check={clean} onFix={vi.fn()} onFixAll={vi.fn()} {...base} />);
    expect(screen.queryByRole('button', { name: 'Fix all safe' })).toBeNull();
  });
});
