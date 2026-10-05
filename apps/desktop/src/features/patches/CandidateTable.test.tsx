import { fireEvent, render, screen, within } from '@testing-library/preact';
import { loadFixture } from 'rimstudio-testkit';
import type { ConvertScanDto } from 'rimstudio-ipc-types';
import { describe, expect, it, vi } from 'vitest';
import { CandidateTable } from './CandidateTable';

const scan = loadFixture<ConvertScanDto>('designer_convert_scan');

describe('CandidateTable', () => {
  it('shows the label, the definition name, the status and the open questions', () => {
    render(
      <CandidateTable
        candidates={scan.candidates}
        focused="OH_G41m"
        onFocus={() => {}}
        checked={new Set()}
        onCheck={() => {}}
      />,
    );
    const row = screen.getByText('OH_G41m').closest('tr') as HTMLElement;
    expect(within(row).getByText('Gewehr 41 (m)')).toBeTruthy();
    expect(within(row).getByText('Not converted')).toBeTruthy();
    expect(within(row).getByText('Ranged / Gun')).toBeTruthy();
    expect(within(row).getByText('9')).toBeTruthy();
    expect(row.getAttribute('aria-selected')).toBe('true');
  });

  it('focuses a weapon by its row and checks it by its box', () => {
    const onFocus = vi.fn();
    const onCheck = vi.fn();
    render(
      <CandidateTable
        candidates={scan.candidates}
        focused={undefined}
        onFocus={onFocus}
        checked={new Set()}
        onCheck={onCheck}
      />,
    );
    fireEvent.click(screen.getByText('OH_G41w'));
    expect(onFocus).toHaveBeenCalledWith('OH_G41w');
    fireEvent.click(screen.getByRole('checkbox', { name: 'Convert Gewehr 41 (w)' }));
    expect(onCheck).toHaveBeenCalledWith('OH_G41w', true);
    expect(onFocus).toHaveBeenCalledTimes(1);
  });

  it('shows an empty text', () => {
    render(
      <CandidateTable
        candidates={[]}
        focused={undefined}
        onFocus={() => {}}
        checked={new Set()}
        onCheck={() => {}}
      />,
    );
    expect(screen.getByText('No weapons')).toBeTruthy();
  });
});
