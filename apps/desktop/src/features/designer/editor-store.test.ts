import { describe, expect, it } from 'vitest';
import type { DraftDto, PreviewDto } from 'rimstudio-ipc-types';
import { createEditorStore } from './editor-store';
import { typed } from './model/draft';
import { cloneEntry, fixture, installTransport, manualScheduler, settle } from './testSupport';

function setup(handlers: Parameters<typeof installTransport>[0] = {}) {
  const transport = installTransport(handlers);
  const clock = manualScheduler();
  const editor = createEditorStore({ projectId: () => 'p-1', scheduler: clock.scheduler });
  return { transport, clock, editor };
}

const count = (calls: ReadonlyArray<{ name: string }>, name: string): number =>
  calls.filter((c) => c.name === name).length;

describe('editor store', () => {
  it('opens a draft and shows the recorded preview with the real numbers', async () => {
    const { clock, editor, transport } = setup();
    editor.open(cloneEntry());
    clock.runFrames();
    await settle();
    expect(editor.saveState.value).toBe('saved');
    const dps = editor.preview.value?.readouts.find((r) => r.key === 'dps');
    expect(dps?.value).toBe(6.875);
    expect(count(transport.calls, 'designer_preview')).toBe(1);
  });

  it('coalesces edits into one preview per animation frame', async () => {
    const { clock, editor, transport } = setup();
    editor.open(cloneEntry());
    clock.runFrames();
    await settle();
    editor.setField('/ranged/damage', typed(23));
    editor.setField('/ranged/damage', typed(24));
    editor.setField('/ranged/damage', typed(25));
    expect(clock.pending().frames).toBe(1);
    clock.runFrames();
    await settle();
    expect(count(transport.calls, 'designer_preview')).toBe(2);
    const sent = transport.calls.filter((c) => c.name === 'designer_preview').at(-1)?.request as {
      draft: { spec: { ranged: { damage: { value: number } } } };
    };
    expect(sent.draft.spec.ranged.damage.value).toBe(25);
  });

  it('asks for the fit, the diff and the structure after a short idle', async () => {
    const { clock, editor, transport } = setup();
    editor.open(cloneEntry());
    clock.runTimers();
    await settle();
    expect(editor.fit.value?.perStat.length).toBeGreaterThan(0);
    expect(editor.diff.value?.source).toBe('Gun_BoltActionRifle');
    expect(count(transport.calls, 'designer_clone_diff')).toBe(1);
    expect(editor.structure.value).toBeDefined();
  });

  it('keeps the newest preview when an older call answers last', async () => {
    const answers: Array<(value: PreviewDto) => void> = [];
    const { clock, editor } = setup({
      designer_preview: () => new Promise<PreviewDto>((resolve) => answers.push(resolve)),
    });
    editor.open(cloneEntry());
    clock.runFrames();
    editor.setField('/mass', typed(4));
    clock.runFrames();
    const older = fixture<PreviewDto>('designer_preview');
    const newer = {
      ...older,
      readouts: [{ key: 'dps', group: 'ranged', unit: 'damage-per-second', value: 99 }],
    } as PreviewDto;
    answers[1]?.(newer);
    await settle();
    answers[0]?.(older);
    await settle();
    expect(editor.preview.value?.readouts[0]?.value).toBe(99);
  });

  it('saves on blur and after the idle, and reports the state', async () => {
    const { clock, editor, transport } = setup();
    editor.open(cloneEntry());
    editor.setField('/ranged/damage', typed(30));
    expect(editor.saveState.value).toBe('dirty');
    await editor.flush();
    expect(editor.saveState.value).toBe('saved');
    expect(editor.entryId.value).toBe('d-test');
    expect(count(transport.calls, 'designer_draft_save')).toBe(1);
    editor.setField('/ranged/damage', typed(31));
    expect(clock.pending().timers).toContain(800);
    clock.runTimers();
    await settle();
    expect(count(transport.calls, 'designer_draft_save')).toBe(2);
  });

  it('shows a save failure and stays dirty for another try', async () => {
    const { editor } = setup({
      designer_draft_save: () => {
        throw { code: 'designer.draft-not-found', message: 'gone', errorId: 'e-1' };
      },
    });
    editor.open(cloneEntry());
    editor.setField('/mass', typed(5));
    await editor.flush();
    expect(editor.saveState.value).toBe('error');
    expect(editor.error.value?.code).toBe('designer.draft-not-found');
  });

  it('fills only empty fields from the suggestions and never replaces a typed value', async () => {
    const { clock, editor } = setup();
    editor.open(cloneEntry());
    clock.runFrames();
    await settle();
    editor.setField('/ranged/burstCount', undefined);
    const before = editor.draft.value?.spec.ranged?.damage;
    const filled = editor.fillFromEstimate();
    expect(filled).toBeGreaterThan(0);
    expect(editor.draft.value?.spec.ranged?.damage).toEqual(before);
    expect(editor.draft.value?.spec.ranged?.burstCount?.source).toBe('suggested');
    expect(editor.draft.value?.spec.ranged?.warmup?.source).toBe('anchor');
  });

  it('takes the suggested structure of a new weapon', async () => {
    const { clock, editor } = setup();
    editor.open({ ...cloneEntry(), draft: fixture('designer-draft-melee'), kind: 'melee' });
    clock.runTimers();
    await settle();
    expect(editor.structure.value?.filled).toBeDefined();
    editor.applyStructure();
    expect(editor.saveState.value).toBe('dirty');
  });

  it('closes the draft and drops the results', () => {
    const { editor } = setup();
    editor.open(cloneEntry());
    editor.close();
    expect(editor.draft.value).toBeUndefined();
    expect(editor.saveState.value).toBe('idle');
  });

  it('asks the backend for an own projectile and takes the draft it answers with', async () => {
    const own = fixture<{ draft: DraftDto; notes?: string[] }>('designer-fields-projectile-own');
    const { clock, editor, transport } = setup({ designer_projectile_own: () => own });
    editor.open(cloneEntry());
    clock.runFrames();
    await settle();
    await editor.setOwnProjectile(true);
    const sent = transport.calls.find((c) => c.name === 'designer_projectile_own')?.request as {
      own: boolean;
    };
    expect(sent.own).toBe(true);
    expect(editor.draft.value).toEqual(own.draft);
    expect(editor.saveState.value).toBe('dirty');
    expect(editor.ownBusy.value).toBe(false);
  });

  it('keeps edits made while the projectile was being built and takes only its projectile', async () => {
    const own = fixture<{ draft: DraftDto }>('designer-fields-projectile-own');
    let release: (value: unknown) => void = () => undefined;
    const { editor } = setup({
      designer_projectile_own: () => new Promise((resolve) => (release = resolve)),
    });
    editor.open(cloneEntry());
    const pending = editor.setOwnProjectile(true);
    editor.setField('/mass', typed(9));
    release(own);
    await pending;
    expect(editor.draft.value?.spec.mass).toEqual(typed(9));
    expect(editor.draft.value?.spec.ranged?.projectile).toEqual(own.draft.spec.ranged?.projectile);
  });

  it('shows the error of a refused projectile change and leaves the draft alone', async () => {
    const { editor } = setup({
      designer_projectile_own: () => {
        throw { code: 'designer.invalid-draft', message: 'not a gun', errorId: 'e-9' };
      },
    });
    editor.open(cloneEntry());
    const before = editor.draft.value;
    await editor.setOwnProjectile(true);
    expect(editor.draft.value).toBe(before);
    expect(editor.error.value?.code).toBe('designer.invalid-draft');
    expect(editor.ownBusy.value).toBe(false);
  });
});
