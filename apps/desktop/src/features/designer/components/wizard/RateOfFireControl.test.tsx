import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { RateOfFireControl } from './RateOfFireControl';
import { findArchetype } from './wizard-model';
import { catalogFixture } from './wizardTestSupport';

function setup(id: string, value: Parameters<typeof RateOfFireControl>[0]['value']) {
  const catalog = catalogFixture();
  const archetype = findArchetype(catalog, id);
  if (!archetype) throw new Error('missing');
  const onChange = vi.fn();
  renderWithProviders(
    <RateOfFireControl archetype={archetype} catalog={catalog} value={value} onChange={onChange} />,
  );
  return onChange;
}

describe('RateOfFireControl', () => {
  it('shows rounds per minute and the time between shots for a gun', () => {
    setup('rifle/assault', { class: 'medium' });
    expect(screen.getByRole('status').textContent).toBe(
      '650 rounds per minute, one shot every 0.09 seconds',
    );
    expect(screen.getByRole('slider', { name: 'Rounds per minute' })).toBeTruthy();
  });

  it('sends a number of rounds per minute when the slider moves', () => {
    const onChange = setup('rifle/assault', { class: 'medium' });
    fireEvent.input(screen.getByRole('slider', { name: 'Rounds per minute' }), {
      target: { value: '800' },
    });
    expect(onChange).toHaveBeenCalledWith({ rpm: 800 });
  });

  it('sends a class from the quick buttons', () => {
    const onChange = setup('rifle/assault', { rpm: 700 });
    fireEvent.click(screen.getByRole('radio', { name: /^Fast/ }));
    expect(onChange).toHaveBeenCalledWith({ class: 'fast' });
    expect(screen.getByRole('status').textContent).toContain('700 rounds per minute');
  });

  it('shows only the classes for a melee weapon', () => {
    setup('sword/long', { class: 'fast' });
    expect(screen.queryByRole('slider')).toBeNull();
    expect(screen.getByRole('radio', { name: 'Fast' }).getAttribute('aria-checked')).toBe('true');
  });
});
