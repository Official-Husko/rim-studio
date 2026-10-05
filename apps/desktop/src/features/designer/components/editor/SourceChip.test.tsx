import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { SourceChip } from './SourceChip';

describe('SourceChip', () => {
  it.each([
    ['typed', 'Typed'],
    ['suggested', 'Suggested'],
    ['anchor', 'Anchor'],
    ['answered', 'Answered'],
  ] as const)('says %s in words', (source, text) => {
    renderWithProviders(<SourceChip source={source} />);
    expect(screen.getByText(text)).toBeTruthy();
  });
});
