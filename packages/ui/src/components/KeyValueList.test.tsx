import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { KeyValueList } from './KeyValueList';

describe('KeyValueList', () => {
  it('renders every pair as a term and a description', () => {
    render(
      <KeyValueList
        label="Weapon"
        items={[
          { key: 'defName', value: 'Gun_Carbine', mono: true },
          { key: 'Kind', value: 'Ranged' },
        ]}
      />,
    );
    expect(screen.getByText('defName').tagName).toBe('DT');
    expect(screen.getByText('Gun_Carbine').tagName).toBe('DD');
    expect(screen.getByText('Gun_Carbine').getAttribute('class')).toContain('font-mono');
    expect(screen.getByText('Ranged')).toBeTruthy();
  });

  it('renders an empty list without throwing', () => {
    const { container } = render(<KeyValueList items={[]} />);
    expect(container.querySelectorAll('dt').length).toBe(0);
  });
});
