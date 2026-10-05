import { describe, expect, it } from 'vitest';
import { formatBytes, formatDuration, formatNumber, plural } from './index';

describe('formatNumber', () => {
  it('groups and limits decimals', () => {
    expect(formatNumber(1234567.891)).toBe('1,234,567.89');
    expect(formatNumber(0.5, 0)).toBe('1');
    expect(formatNumber(2, 3)).toBe('2');
  });
  it('shows a dash for non finite values', () => {
    expect(formatNumber(Number.NaN)).toBe('-');
    expect(formatNumber(Infinity)).toBe('-');
  });
});

describe('formatBytes', () => {
  it.each([
    [0, '0 B'],
    [1023, '1,023 B'],
    [1024, '1 KiB'],
    [1536, '1.5 KiB'],
    [5 * 1024 * 1024, '5 MiB'],
    [250 * 1024 * 1024, '250 MiB'],
  ])('%d bytes is %s', (input, expected) => {
    expect(formatBytes(input)).toBe(expected);
  });
  it('rejects negative sizes', () => {
    expect(formatBytes(-1)).toBe('-');
  });
});

describe('formatDuration', () => {
  it.each([
    [420, '420 ms'],
    [1200, '1.2 s'],
    [42_000, '42 s'],
    [125_000, '2 min 5 s'],
    [120_000, '2 min'],
    [3_780_000, '1 h 3 min'],
    [3_600_000, '1 h'],
  ])('%d ms is %s', (input, expected) => {
    expect(formatDuration(input)).toBe(expected);
  });
});

describe('plural', () => {
  it('chooses by count', () => {
    expect(plural(1, 'file', 'files')).toBe('file');
    expect(plural(0, 'file', 'files')).toBe('files');
    expect(plural(2, 'file', 'files')).toBe('files');
  });
});
