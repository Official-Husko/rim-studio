import { describe, expect, it } from 'vitest';
import { readoutLabel, readoutValue, signed, statLabel, unitText } from './labels';

describe('labels', () => {
  it('names the readouts and shows an unknown key as it is', () => {
    expect(readoutLabel('cycle-time')).toBe('Cycle time');
    expect(readoutLabel('hit-dps-12')).toBe('Hit adjusted DPS at 12 tiles');
    expect(readoutLabel('something-new')).toBe('something-new');
  });

  it('formats values, deltas and units', () => {
    expect(readoutValue(undefined)).toBe('-');
    expect(readoutValue(6.875)).toBe('6.875');
    expect(signed(1.25)).toBe('+1.25');
    expect(signed(-0.5)).toBe('-0.5');
    expect(signed(0)).toBe('0');
    expect(unitText('damage-per-second')).toBe('dmg/s');
  });

  it('names stats and prettifies the unknown ones', () => {
    expect(statLabel('swing_damage')).toBe('Swing damage');
    expect(statLabel('some_new_stat')).toBe('some new stat');
  });
});
