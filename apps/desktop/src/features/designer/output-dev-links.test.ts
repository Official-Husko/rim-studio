import { afterEach, describe, expect, it } from 'vitest';
import { parseOutputLinks, runOutputLinks } from './output-dev-links';
import { outputEntry, setupOutput } from './output-testSupport';

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  stop = undefined;
});

describe('output dev links', () => {
  it('reads the steps in order and skips what it does not know', () => {
    const steps = parseOutputLinks(
      '#/weapons?project=P&out=ce|all|choice=/ce/ammoSet:AmmoSet_X|number=/ce/shotSpread:0.2|file=1|nonsense|apply|confirm',
    );
    expect(steps.map((s) => s.kind)).toEqual([
      'ce',
      'mode',
      'choice',
      'number',
      'file',
      'apply',
      'confirm',
    ]);
    expect(steps[2]).toEqual({ kind: 'choice', pointer: '/ce/ammoSet', value: 'AmmoSet_X' });
    expect(steps[3]).toEqual({ kind: 'number', pointer: '/ce/shotSpread', value: 0.2 });
  });

  it('reads nothing without the parameter', () => {
    expect(parseOutputLinks('#/weapons')).toEqual([]);
  });

  it('runs the whole flow once the first plan has arrived', async () => {
    const setup = setupOutput();
    const stopOutput = setup.output.start();
    setup.editor.open(outputEntry('vanilla'));
    const steps = parseOutputLinks(
      '#/weapons?out=ce|all|choice=/ce/ammoSet:AmmoSet_303British_SB|choice=/ce/weaponTagClass:CE_AI_SR|number=/ce/shotSpread:0.165|number=/ce/magazineSize:7|number=/ce/toolPenetration/stock/blunt:2.56|number=/ce/toolPenetration/barrel/blunt:2.56|apply',
    );
    const stopLinks = runOutputLinks(setup.output, setup.editor, steps);
    stop = () => {
      stopLinks();
      stopOutput();
    };
    for (let i = 0; i < 12; i += 1) await setup.flush();
    expect(setup.editor.draft.value?.spec.ce?.ammoSet).toBe('AmmoSet_303British_SB');
    expect(setup.output.plan.value?.files).toHaveLength(3);
    expect(setup.output.apply.value.phase).toBe('confirm');
  });
});
