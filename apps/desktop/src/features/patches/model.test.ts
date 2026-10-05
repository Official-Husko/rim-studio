import type { AskItemDto, ConvertCandidateDto, DiagnosticDto } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import {
  answerOf,
  buildRequest,
  emptyAnswers,
  projectileOf,
  isEmptyAnswers,
  parseDerived,
  splitDiagnostics,
  toAnswersDto,
  toGroupAnswers,
  withAnswer,
} from './model';

const ask = (field: string, kind: AskItemDto['kind'] = 'number'): AskItemDto => ({
  field,
  label: field,
  kind,
  options: [],
});

const candidate: ConvertCandidateDto = {
  defName: 'OH_G41m',
  label: 'Gewehr 41 (m)',
  kind: 'ranged',
  status: 'not-converted',
  reason: 'can be converted',
  asks: [],
  family: 'ranged/Gun/Bullet_MauserRifle',
};

describe('projectileOf', () => {
  it('reads the default projectile of the family key', () => {
    expect(projectileOf(candidate)).toBe('Bullet_MauserRifle');
    expect(projectileOf({ ...candidate, family: 'melee/Knife' })).toBe('');
    expect(projectileOf({ ...candidate, family: undefined })).toBe('');
  });
});

describe('answers', () => {
  it('stores and removes answers by ask field', () => {
    let set = withAnswer(undefined, ask('/ce/ammoSet', 'choice'), 'AmmoSet_303British');
    set = withAnswer(set, ask('/ce/oneHanded', 'flag'), false);
    set = withAnswer(set, ask('/ce/shotSpread'), 0.1);
    expect(answerOf(set, ask('/ce/ammoSet', 'choice'))).toBe('AmmoSet_303British');
    expect(answerOf(set, ask('/ce/oneHanded', 'flag'))).toBe(false);
    expect(answerOf(set, ask('/ce/shotSpread'))).toBe(0.1);
    set = withAnswer(set, ask('/ce/shotSpread'), undefined);
    set = withAnswer(set, ask('/ce/ammoSet', 'choice'), '');
    expect(answerOf(set, ask('/ce/shotSpread'))).toBeUndefined();
    expect(answerOf(set, ask('/ce/ammoSet', 'choice'))).toBeUndefined();
    expect(isEmptyAnswers(emptyAnswers())).toBe(true);
    expect(isEmptyAnswers(set)).toBe(false);
  });

  it('maps numeric answers to overrides and tool penetration entries', () => {
    let set = withAnswer(undefined, ask('/ce/shotSpread'), 0.1);
    set = withAnswer(set, ask('/ce/magazineSize'), 5);
    set = withAnswer(set, ask('/ce/toolPenetration/stock/blunt'), 2.5);
    set = withAnswer(set, ask('/ce/toolPenetration/stock/sharp'), 1);
    set = withAnswer(set, ask('/ce/toolPenetration/barrel/blunt'), 1.7);
    const group = toGroupAnswers(set);
    expect(group.overrides).toEqual({
      magazineSize: { value: 5, source: 'answered' },
      shotSpread: { value: 0.1, source: 'answered' },
    });
    expect(group.toolPenetration).toEqual([
      {
        tool: 'barrel',
        blunt: { value: 1.7, source: 'answered' },
      },
      {
        tool: 'stock',
        sharp: { value: 1, source: 'answered' },
        blunt: { value: 2.5, source: 'answered' },
      },
    ]);
  });

  it('keeps the two flags of the typed answers false unless answered', () => {
    const dto = toAnswersDto(withAnswer(undefined, ask('/ce/beltFed', 'flag'), true));
    expect(dto.beltFed).toBe(true);
    expect(dto.oneHanded).toBeUndefined();
    expect(dto.overrides).toEqual({ oneHanded: false, beltFed: false });
  });
});

describe('buildRequest', () => {
  it('sends a group only for a family that has answers', () => {
    const family = withAnswer(undefined, ask('/ce/ammoSet', 'choice'), 'AmmoSet_303British');
    const withGroup = buildRequest(candidate, undefined, family);
    expect(withGroup.groups).toEqual([
      { family: 'ranged/Gun/Bullet_MauserRifle', answers: { ammoSet: 'AmmoSet_303British' } },
    ]);
    expect(buildRequest(candidate, undefined, emptyAnswers()).groups).toBeUndefined();
    expect(buildRequest({ ...candidate, family: '' }, undefined, family).groups).toBeUndefined();
  });
});

describe('diagnostics', () => {
  const d = (code: string, message: string, extra: Partial<DiagnosticDto> = {}): DiagnosticDto => ({
    code,
    severity: 'info',
    message,
    ...extra,
  });

  it('reads a derived value from its sentence', () => {
    const v = parseDerived(
      d(
        'ce.derived-value',
        'range = 52.92 was predicted (Ratio) from 4 of the converted weapons in the library, rated rough, typical error about 31 percent; check it',
        { field: 'range' },
      ),
    );
    expect(v).toMatchObject({
      field: 'range',
      value: '52.92',
      predictor: 'Ratio',
      rating: 'rough',
    });
    const plain = parseDerived(
      d(
        'designer.convert-derived',
        'ce.bulk = 12.49 was predicted (Ratio) from 4 of your converted weapons; check it',
        {
          field: 'ce.bulk',
        },
      ),
    );
    expect(plain).toMatchObject({ field: 'bulk', value: '12.49', predictor: 'Ratio' });
    expect(plain.rating).toBeUndefined();
  });

  it("splits a plan's diagnostics by where they are shown", () => {
    const split = splitDiagnostics([
      d('designer.convert-needs-answer', 'q'),
      d('ce.derived-value', 'a = 1 was copied'),
      d('ce.not-checked', 'CEP010 was not checked'),
      d('ce.about-suggestion', 'hint', { severity: 'hint' }),
      d('ce.x', 'err', { severity: 'error' }),
    ]);
    expect(split.open).toHaveLength(1);
    expect(split.derived).toHaveLength(1);
    expect(split.notChecked).toHaveLength(1);
    expect(split.other.map((x) => x.severity)).toEqual(['error', 'hint']);
  });
});
