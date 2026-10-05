import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { ScanBar } from './ScanBar';

const counts = { notConverted: 3, alreadyCe: 2, unsupportedKind: 1, targetNotFound: 0 };

describe('ScanBar', () => {
  it('shows the counts and hides the empty ones', () => {
    render(
      <ScanBar
        counts={counts}
        includeConverted
        onIncludeConverted={() => {}}
        onRescan={() => {}}
        rescanning={false}
        convertible={3}
        checked={0}
        onCheckAll={() => {}}
        onConvert={() => {}}
      />,
    );
    expect(screen.getByText('3 not converted')).toBeTruthy();
    expect(screen.getByText('2 already CE')).toBeTruthy();
    expect(screen.getByText('1 unsupported')).toBeTruthy();
    expect(screen.queryByText(/target not found/)).toBeNull();
  });

  it('selects all, converts the checked and rescans', () => {
    const onCheckAll = vi.fn();
    const onConvert = vi.fn();
    const onRescan = vi.fn();
    const onInclude = vi.fn();
    render(
      <ScanBar
        counts={counts}
        includeConverted
        onIncludeConverted={onInclude}
        onRescan={onRescan}
        rescanning={false}
        convertible={3}
        checked={2}
        onCheckAll={onCheckAll}
        onConvert={onConvert}
      />,
    );
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'Select all 3 weapons that can be converted' }),
    );
    expect(onCheckAll).toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Convert 2 weapons' }));
    expect(onConvert).toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Scan again' }));
    expect(onRescan).toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole('switch', { name: 'Show weapons that are already converted' }),
    );
    expect(onInclude).toHaveBeenCalledWith(false);
  });

  it('disables the convert button without a checked weapon', () => {
    render(
      <ScanBar
        counts={counts}
        includeConverted={false}
        onIncludeConverted={() => {}}
        onRescan={() => {}}
        rescanning={false}
        convertible={0}
        checked={0}
        onCheckAll={() => {}}
        onConvert={() => {}}
      />,
    );
    expect(screen.getByRole('button', { name: 'Convert 0 weapons' }).hasAttribute('disabled')).toBe(
      true,
    );
  });
});
