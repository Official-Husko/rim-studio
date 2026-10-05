import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { Meter, type FitLevel } from './Meter';

const base = {
  min: 0,
  max: 20,
  p50: [8, 12] as [number, number],
  p80: [5, 15] as [number, number],
  label: 'DPS',
  unit: 'dps',
};

describe('Meter', () => {
  it('is a meter with value, range and a spoken level', () => {
    render(<Meter {...base} value={10} level="typical" />);
    const meter = screen.getByRole('meter', { name: 'DPS' });
    expect(meter.getAttribute('aria-valuenow')).toBe('10');
    expect(meter.getAttribute('aria-valuemin')).toBe('0');
    expect(meter.getAttribute('aria-valuemax')).toBe('20');
    expect(meter.getAttribute('aria-valuetext')).toBe('10 dps, typical');
  });

  it('draws the bands and positions the caret by percentage', () => {
    const { container } = render(<Meter {...base} value={15} level="plausible" />);
    const caret = container.querySelector<HTMLElement>('[data-marker="value"]');
    expect(caret?.style.left).toBe('75%');
    expect(container.querySelector<HTMLElement>('[data-band="p50"]')?.style.left).toBe('40%');
    expect(container.querySelector<HTMLElement>('[data-band="p50"]')?.style.width).toBe('20%');
    expect(container.querySelector('[data-band="p80"]')).not.toBeNull();
  });

  it('clamps a value outside the scale to the ends', () => {
    const { container } = render(<Meter {...base} value={99} level="unusual" />);
    expect(container.querySelector<HTMLElement>('[data-marker="value"]')?.style.left).toBe('100%');
  });

  it.each<[FitLevel, string]>([
    ['typical', 'text-success'],
    ['plausible', 'text-info'],
    ['unusual', 'text-warning'],
  ])('uses a non red colour for %s', (level, cls) => {
    const { container } = render(<Meter {...base} value={10} level={level} />);
    expect(container.innerHTML).toContain(cls);
    expect(container.innerHTML).not.toContain('text-danger');
  });

  it('accepts translated text', () => {
    render(<Meter {...base} value={10} level="typical" levelText="typisch" valueText="10,0 dps" />);
    expect(screen.getByText('typisch')).toBeTruthy();
  });
});
