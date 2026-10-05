import { fireEvent, render, screen } from '@testing-library/preact';
import { loadFixture } from 'rimstudio-testkit';
import type { ConvertScanDto, WritePlanDto } from 'rimstudio-ipc-types';
import { describe, expect, it, vi } from 'vitest';
import { ApplyReview, fileRows } from './ApplyReview';
import type { ReviewItem } from './applyStore';

const candidates = loadFixture<ConvertScanDto>('designer_convert_scan').candidates;
const ready = loadFixture<WritePlanDto>('patches-plan-ready');
const open = loadFixture<WritePlanDto>('patches-plan-open');

function item(index: number, plan: WritePlanDto): ReviewItem {
  const candidate = candidates[index];
  if (!candidate) throw new Error('fixture');
  return { candidate, entry: { key: 'k', phase: 'ready', plan } };
}

describe('ApplyReview', () => {
  it('lists the files once, the LoadFolders.xml edit and the backup choice', () => {
    const items = [item(0, ready), item(1, ready)];
    expect(fileRows(items).map((r) => r.path)).toEqual([
      'Compat/CombatExtended/Patches/gewehr41_Weapons_Ranged.xml',
      'LoadFolders.xml',
    ]);
    const onBackup = vi.fn();
    render(<ApplyReview items={items} backup dryApply onBackup={onBackup} onDryApply={() => {}} />);
    expect(screen.getByText('2 weapons will be converted.')).toBeTruthy();
    expect(screen.getAllByText('shared by 2 weapons, written one after the other')).toHaveLength(2);
    expect(screen.getByText(/A new LoadFolders.xml is created/)).toBeTruthy();
    expect(screen.getByText(/project backups folder inside the app data folder/)).toBeTruthy();
    fireEvent.click(screen.getByRole('switch', { name: 'Back up files that are replaced' }));
    expect(onBackup).toHaveBeenCalledWith(false);
  });

  it('keeps a weapon with open questions out and says why', () => {
    render(
      <ApplyReview
        items={[item(0, ready), item(1, open)]}
        backup
        dryApply
        onBackup={() => {}}
        onDryApply={() => {}}
      />,
    );
    expect(screen.getByText('1 weapon will be converted.')).toBeTruthy();
    expect(screen.getByText('1 weapon cannot be applied yet')).toBeTruthy();
    expect(screen.getByText('9 questions are still open', { exact: false })).toBeTruthy();
  });

  it('says every file is new when nothing is replaced', () => {
    render(
      <ApplyReview
        items={[item(0, ready)]}
        backup
        dryApply
        onBackup={() => {}}
        onDryApply={() => {}}
      />,
    );
    expect(screen.getByText('Every file is new, so there is nothing to back up.')).toBeTruthy();
  });
});
