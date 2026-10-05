import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { SectionHeading } from './SectionHeading';

describe('SectionHeading', () => {
  it('renders a level three heading', () => {
    renderWithProviders(<SectionHeading>Range</SectionHeading>);
    expect(screen.getByRole('heading', { level: 3, name: 'Range' })).toBeTruthy();
  });
});
