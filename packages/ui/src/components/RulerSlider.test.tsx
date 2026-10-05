import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { RulerSlider } from './RulerSlider';

const base = { min: 0, max: 40, step: 1, label: 'Damage', unit: 'dmg' };

describe('RulerSlider', () => {
  it('is a slider with a name, range and value text', () => {
    render(<RulerSlider {...base} value={12} onValueChange={() => {}} />);
    const slider = screen.getByRole('slider', { name: 'Damage' }) as HTMLInputElement;
    expect(slider.min).toBe('0');
    expect(slider.max).toBe('40');
    expect(slider.step).toBe('1');
    expect(slider.value).toBe('12');
    expect(slider.getAttribute('aria-valuetext')).toBe('12 dmg');
  });

  it('reports a new value as a number', () => {
    const onValueChange = vi.fn();
    render(<RulerSlider {...base} value={12} onValueChange={onValueChange} />);
    fireEvent.input(screen.getByRole('slider'), { target: { value: '18' } });
    expect(onValueChange).toHaveBeenCalledWith(18);
  });

  it('draws marks, bands and the suggested marker', () => {
    const { container } = render(
      <RulerSlider
        {...base}
        value={12}
        onValueChange={() => {}}
        marks={[
          { value: 5, label: 'p10 5' },
          { value: 20, label: 'median 20' },
          { value: 12, label: 'this item', kind: 'item' },
        ]}
        p50={[10, 20]}
        p80={[5, 30]}
        suggested={14}
      />,
    );
    expect(container.querySelectorAll('[data-mark]')).toHaveLength(3);
    expect(container.querySelector<HTMLElement>('[data-band="p50"]')?.style.left).toBe('25%');
    expect(screen.getByRole('img', { name: 'Suggested 14 dmg' })).toBeTruthy();
  });

  it('labels the major ticks across the range', () => {
    render(<RulerSlider {...base} value={12} onValueChange={() => {}} />);
    for (const text of ['0', '10', '20', '30', '40'])
      expect(screen.getAllByText(text).length).toBeGreaterThan(0);
  });

  it('can be disabled', () => {
    render(<RulerSlider {...base} value={1} disabled onValueChange={() => {}} />);
    expect((screen.getByRole('slider') as HTMLInputElement).disabled).toBe(true);
  });
});
