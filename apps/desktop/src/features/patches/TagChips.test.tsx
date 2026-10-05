import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { TagChips } from './TagChips';

describe('TagChips', () => {
  it('lists the tags and the classes', () => {
    render(<TagChips tags={['Gun', 'CE_AI_SR']} classes={['Ranged']} />);
    const list = screen.getByRole('list', { name: 'Tags and classes' });
    expect(list.querySelectorAll('li').length).toBe(3);
    expect(screen.getByText('CE_AI_SR')).toBeTruthy();
    expect(screen.getByText('Ranged').closest('li')?.getAttribute('title')).toBe('Weapon class');
  });

  it('shows a dash when there is none', () => {
    render(<TagChips tags={[]} classes={[]} />);
    expect(screen.getByText('-')).toBeTruthy();
  });

  it('cuts a table cell off after the limit and counts the rest', () => {
    render(<TagChips tags={['Gun', 'CE_AI_SR']} classes={['Ranged', 'LongShots']} limit={2} />);
    expect(screen.getByText('+2')).toBeTruthy();
    expect(screen.getByText('+2').closest('li')?.getAttribute('title')).toBe('Ranged, LongShots');
    expect(screen.queryByText('Ranged')).toBeNull();
  });
});
