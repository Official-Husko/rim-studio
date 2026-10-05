import { describe, expect, it } from 'vitest';
import type { DiagnosticDto } from 'rimstudio-ipc-types';
import {
  chipKind,
  diagnosticsFor,
  findFieldElement,
  focusField,
  groupByField,
  joinList,
  newDraft,
  splitList,
  suggested,
  suggestionsByField,
  typed,
} from './draft';

const diag = (field: string | undefined, code: string): DiagnosticDto => ({
  code,
  severity: 'error',
  message: code,
  ...(field ? { field } : {}),
});

describe('draft helpers', () => {
  it('starts an empty draft of a kind in simple mode', () => {
    const draft = newDraft('melee', 'Mod_Sword', 'sword');
    expect(draft.kind).toBe('melee');
    expect(draft.calibration).toBe('simple');
    expect(draft.spec.identity.defName).toBe('Mod_Sword');
  });

  it('marks numbers as typed or suggested', () => {
    expect(typed(3)).toEqual({ value: 3, source: 'typed' });
    expect(suggested(3).source).toBe('suggested');
    expect(chipKind('answered')).toBe('neutral');
    expect(chipKind('anchor')).toBe('anchor');
  });

  it('splits and joins comma lists', () => {
    expect(splitList(' Cut, Stab ,, ')).toEqual(['Cut', 'Stab']);
    expect(joinList(['Cut', 'Stab'])).toBe('Cut, Stab');
    expect(joinList(undefined)).toBe('');
  });

  it('groups diagnostics by field and finds those below a pointer', () => {
    const grouped = groupByField([
      diag('/tools/0/power', 'a'),
      diag('/tools', 'b'),
      diag(undefined, 'c'),
    ]);
    expect(grouped.get('')?.length).toBe(1);
    expect(
      diagnosticsFor(grouped, '/tools')
        .map((d) => d.code)
        .sort(),
    ).toEqual(['a', 'b']);
    expect(diagnosticsFor(grouped, '/mass')).toEqual([]);
  });

  it('keys suggestions by field', () => {
    const map = suggestionsByField([
      { field: '/mass', stat: 'mass', level: 'all', predictor: 'median', n: 3, locked: false },
    ]);
    expect(map.get('/mass')?.stat).toBe('mass');
  });

  it('finds the element of a pointer or of the nearest field around it', () => {
    document.body.innerHTML =
      '<div data-field="/tools/0/power"><input id="a"></div><div data-field="/mass"><input id="b"></div>';
    expect(findFieldElement(document.body, '/mass')?.dataset.field).toBe('/mass');
    expect(findFieldElement(document.body, '/tools')?.dataset.field).toBe('/tools/0/power');
    expect(findFieldElement(document.body, '/mass/extra')?.dataset.field).toBe('/mass');
    expect(findFieldElement(document.body, '/nothing')).toBeNull();
    expect(focusField(document.body, '/mass')).toBe(true);
    expect(document.activeElement?.id).toBe('b');
    expect(focusField(document.body, '/nothing')).toBe(false);
  });
});
