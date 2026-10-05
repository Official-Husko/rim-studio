import { describe, expect, it } from 'vitest';
import type { CeSuggestionDto, DesignSpecDto, WritePlanDto } from 'rimstudio-ipc-types';
import { newDraft } from './model/draft';
import {
  acceptedFor,
  derivedPointers,
  effectiveAccepted,
  emptyCeBlock,
  folderOf,
  isCeRequiredMissing,
  nameOf,
  pendingAnswers,
  problemsOf,
  setCeValue,
  sourced,
  toggled,
} from './output-model';
import { fixture } from './testSupport';

const suggestion = () => fixture<CeSuggestionDto>('designer-output-suggest-on');
const spec = (): DesignSpecDto => ({
  ...newDraft('ranged', 'TM_Gun', 'gun').spec,
  ce: emptyCeBlock(),
});

describe('setCeValue', () => {
  it('writes a plain answer', () => {
    const next = setCeValue(spec(), '/ce/ammoSet', 'AmmoSet_X');
    expect(next.ce?.ammoSet).toBe('AmmoSet_X');
    expect(next.ce?.oneHanded).toBe(false);
  });

  it('writes a tool penetration under the tool name and removes it again', () => {
    const one = setCeValue(spec(), '/ce/toolPenetration/stock/blunt', sourced(2, 'answered'));
    const two = setCeValue(one, '/ce/toolPenetration/barrel/sharp', sourced(1, 'typed'));
    expect(two.ce?.toolPenetration).toEqual([
      { tool: 'stock', blunt: { value: 2, source: 'answered' } },
      { tool: 'barrel', sharp: { value: 1, source: 'typed' } },
    ]);
    const cleared = setCeValue(two, '/ce/toolPenetration/stock/blunt', undefined);
    expect(cleared.ce?.toolPenetration).toEqual([
      { tool: 'barrel', sharp: { value: 1, source: 'typed' } },
    ]);
    const empty = setCeValue(cleared, '/ce/toolPenetration/barrel/sharp', undefined);
    expect(empty.ce?.toolPenetration).toBeUndefined();
  });

  it('keeps the other half of a tool entry', () => {
    const a = setCeValue(spec(), '/ce/toolPenetration/stock/blunt', sourced(2, 'typed'));
    const b = setCeValue(a, '/ce/toolPenetration/stock/sharp', sourced(3, 'typed'));
    expect(b.ce?.toolPenetration).toEqual([
      { tool: 'stock', blunt: { value: 2, source: 'typed' }, sharp: { value: 3, source: 'typed' } },
    ]);
  });

  it('does not change its input', () => {
    const before = spec();
    setCeValue(before, '/ce/toolPenetration/stock/blunt', sourced(2, 'typed'));
    expect(before.ce?.toolPenetration).toBeUndefined();
  });
});

describe('accepted values', () => {
  it('finds the derived fields and, with the filter, only the reliable ones', () => {
    const all = derivedPointers(suggestion(), false);
    expect(all).toContain('/ce/bulk');
    expect(all).not.toContain('/ce/shotSpread');
    expect(derivedPointers(suggestion(), true)).toEqual([]);
    expect(derivedPointers(undefined, false)).toEqual([]);
  });

  it('includes the default projectile that follows the ammo set', () => {
    const answered = fixture<CeSuggestionDto>('designer-output-suggest-answered');
    expect(derivedPointers(answered, false)).toContain('/ce/defaultProjectile');
  });

  it('sends nothing for none, an empty list for all, and the pointers otherwise', () => {
    const s = suggestion();
    expect(acceptedFor(true, 'none', [], s)).toBeUndefined();
    expect(acceptedFor(false, 'all', [], s)).toBeUndefined();
    expect(acceptedFor(true, 'all', [], s)).toEqual([]);
    expect(acceptedFor(true, 'custom', ['/ce/bulk'], s)).toEqual(['/ce/bulk']);
    expect(acceptedFor(true, 'custom', [], s)).toBeUndefined();
    expect(acceptedFor(true, 'reliable', [], s)).toBeUndefined();
  });

  it('shows the checkboxes of the effective set', () => {
    const s = suggestion();
    expect(effectiveAccepted('none', ['/ce/bulk'], s)).toEqual([]);
    expect(effectiveAccepted('custom', ['/ce/bulk'], s)).toEqual(['/ce/bulk']);
    expect(effectiveAccepted('all', [], s).length).toBeGreaterThan(3);
    expect(toggled(['/ce/a', '/ce/b'], '/ce/a', false)).toEqual(['/ce/b']);
    expect(toggled(['/ce/a'], '/ce/b', true)).toEqual(['/ce/a', '/ce/b']);
  });
});

describe('plan helpers', () => {
  it('lists each pending answer once', () => {
    const plan = fixture<WritePlanDto>('designer-output-plan-ce-needs-answer');
    const fields = pendingAnswers(plan).map((d) => d.field);
    expect(new Set(fields).size).toBe(fields.length);
    expect(fields).toContain('/ce/ammoSet');
    expect(pendingAnswers(undefined)).toEqual([]);
  });

  it('knows a required Combat Extended field that the plan may still fill', () => {
    const base = { severity: 'error' as const, message: 'm' };
    expect(
      isCeRequiredMissing({ ...base, code: 'design.required-missing', field: '/ce/bulk' }),
    ).toBe(true);
    expect(
      isCeRequiredMissing({ ...base, code: 'design.required-missing', field: '/identity/defName' }),
    ).toBe(false);
    expect(isCeRequiredMissing({ ...base, code: 'design.other', field: '/ce/bulk' })).toBe(false);
  });

  it('keeps the derived values and the lint out of the problems and puts errors first', () => {
    const plan = fixture<WritePlanDto>('designer-output-plan-ce-ready');
    const problems = problemsOf(plan);
    expect(problems.some((d) => d.code === 'ce.derived-value')).toBe(false);
    expect(problems.some((d) => d.code === 'ce.not-checked')).toBe(false);
    const withErrors = problemsOf(
      fixture<WritePlanDto>('designer-output-plan-ce-nothing-accepted'),
    );
    expect(withErrors[0]?.severity).toBe('error');
  });

  it('splits a path', () => {
    expect(folderOf('a/b/c.xml')).toBe('a/b');
    expect(folderOf('LoadFolders.xml')).toBe('');
    expect(nameOf('a/b/c.xml')).toBe('c.xml');
  });
});
