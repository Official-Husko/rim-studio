import { render, screen, within } from '@testing-library/preact';
import { loadFixture } from 'rimstudio-testkit';
import type { LibraryScanResult } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import { DuplicateGroups } from './DuplicateGroups';

const scan = loadFixture<LibraryScanResult>('library-scan-with-custom');
const duplicates = scan.duplicates;
if (!duplicates) throw new Error('fixture');

describe('DuplicateGroups', () => {
  it('lists the kept and skipped copies with their sources and the reason', () => {
    render(<DuplicateGroups duplicates={duplicates} />);
    expect(screen.getByRole('status').textContent).toContain('6 package ids');
    const row = screen.getByText('Mlie.researchtree').closest('tr') as HTMLElement;
    expect(within(row).getAllByText('Research Tree (Continued)').length).toBe(2);
    expect(within(row).getByText('Game Mods')).toBeTruthy();
    expect(within(row).getByText('Workshop')).toBeTruthy();
    expect(
      within(row).getByText('the kept copy comes from a source of higher priority'),
    ).toBeTruthy();
    expect(within(row).getByText('The source with the higher priority wins')).toBeTruthy();
  });

  it('says when the list is capped', () => {
    render(<DuplicateGroups duplicates={{ ...duplicates, total: 60 }} />);
    expect(screen.getByRole('status').textContent).toContain('first 6');
  });

  it('says so when there is none', () => {
    render(<DuplicateGroups duplicates={{ total: 0, skippedTotal: 0, groups: [] }} />);
    expect(screen.getByText('No package id is used by more than one mod.')).toBeTruthy();
  });
});
