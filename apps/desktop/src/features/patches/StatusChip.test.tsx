import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { StatusChip } from './StatusChip';

describe('StatusChip', () => {
  it.each([
    ['not-converted', 'Not converted'],
    ['already-ce', 'Already CE'],
    ['unsupported-kind', 'Unsupported kind'],
    ['target-not-found', 'Target not found'],
  ] as const)('writes the word for %s', (status, text) => {
    render(<StatusChip status={status} />);
    expect(screen.getByText(text)).toBeTruthy();
  });
});
