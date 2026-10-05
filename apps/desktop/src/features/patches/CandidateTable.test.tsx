import { act, fireEvent, render, screen, within } from '@testing-library/preact';
import { resetAnswers, setAnswer } from './answerStore';
import { loadFixture } from 'rimstudio-testkit';
import type { ConvertScanDto } from 'rimstudio-ipc-types';
import { describe, expect, it, vi } from 'vitest';
import { CandidateTable } from './CandidateTable';

const scan = loadFixture<ConvertScanDto>('designer_convert_scan');
const base = scan.candidates[0];
if (!base) throw new Error('fixture');

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
    expect(within(row).getByText('Gun')).toBeTruthy();
    expect(within(row).getByText('+2')).toBeTruthy();
    expect(within(row).getByText('26')).toBeTruthy();
    expect(within(row).getByText('44')).toBeTruthy();
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

  it('narrows the rows by status and by tag and says how many are shown', () => {
    const list = [
      ...scan.candidates,
      {
        ...base,
        defName: 'OH_Done',
        label: 'Done',
        status: 'already-ce' as const,
        tags: ['Pistol'],
        weaponClasses: [],
      },
    ];
    render(
      <CandidateTable
        candidates={list}
        focused={undefined}
        onFocus={() => {}}
        checked={new Set()}
        onCheck={() => {}}
      />,
    );
    expect(screen.getByRole('status').textContent).toBe(`Showing ${list.length} of ${list.length}`);
    fireEvent.change(screen.getByLabelText('Status'), { target: { value: 'already-ce' } });
    expect(screen.getByRole('status').textContent).toBe(`Showing 1 of ${list.length}`);
    expect(screen.getByText('OH_Done')).toBeTruthy();
    fireEvent.change(screen.getByLabelText('Status'), { target: { value: '' } });
    fireEvent.change(screen.getByLabelText('Tag or class'), { target: { value: 'Pistol' } });
    expect(screen.getByRole('status').textContent).toBe(`Showing 1 of ${list.length}`);
    fireEvent.change(screen.getByLabelText('Tag or class'), { target: { value: 'Pistol' } });
    fireEvent.change(screen.getByLabelText('Status'), { target: { value: 'not-converted' } });
    expect(screen.getByText('No weapon matches the filter')).toBeTruthy();
  });

  it('sorts by a number column and flips the direction on a second click', () => {
    const list = [
      { ...base, defName: 'A', label: 'A', vanilla: { damage: 5 } },
      { ...base, defName: 'B', label: 'B', vanilla: { damage: 30 } },
      { ...base, defName: 'C', label: 'C', vanilla: {} },
    ];
    render(
      <CandidateTable
        candidates={list}
        focused={undefined}
        onFocus={() => {}}
        checked={new Set()}
        onCheck={() => {}}
      />,
    );
    const names = () =>
      screen
        .getAllByRole('row')
        .slice(1)
        .map((r) => r.querySelector('span.truncate')?.textContent);
    expect(names()).toEqual(['A', 'B', 'C']);
    fireEvent.click(screen.getByRole('button', { name: 'Damage' }));
    expect(names()).toEqual(['A', 'B', 'C']);
    fireEvent.click(screen.getByRole('button', { name: 'Damage' }));
    expect(names()).toEqual(['B', 'A', 'C']);
  });

  it('counts the questions that are still open after an answer', () => {
    resetAnswers();
    const first = base;
    render(
      <CandidateTable
        candidates={[first]}
        focused={undefined}
        onFocus={() => {}}
        checked={new Set()}
        onCheck={() => {}}
      />,
    );
    expect(screen.getByText('9')).toBeTruthy();
    const ask = first.asks.find((a) => a.field === '/ce/ammoSet');
    if (!ask) throw new Error('fixture');
    act(() => setAnswer(first, 'weapon', ask, 'AmmoSet_556x45mmNATO'));
    expect(screen.getByText('8')).toBeTruthy();
    resetAnswers();
  });
});
