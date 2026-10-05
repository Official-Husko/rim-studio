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
});
