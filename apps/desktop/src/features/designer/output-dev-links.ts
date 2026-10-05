import { effect } from '@preact/signals';
import type { EditorStore } from './editor-store';
import type { AcceptMode } from './output-model';
import { sourced } from './output-model';
import type { OutputStore } from './output-store';

/**
 * Development deep links for the output panel, read from the hash next to the ones of the page,
 * for example #/weapons?project=P&clone=A:B&out=ce|all|choice=/ce/ammoSet:AmmoSet_X|number=/ce/shotSpread:0.2|apply|confirm.
 * Steps run in order once the first plan has arrived. They do nothing in a production build.
 */
export type OutputStep =
  | { kind: 'ce' }
  | { kind: 'mode'; mode: AcceptMode }
  | { kind: 'choice'; pointer: string; value: string }
  | { kind: 'number'; pointer: string; value: number }
  | { kind: 'file'; index: number }
  | { kind: 'apply' }
  | { kind: 'confirm' };

/** Read the steps of the out parameter of a location hash. */
export function parseOutputLinks(hash: string): OutputStep[] {
  const at = hash.indexOf('?');
  const raw = new URLSearchParams(at < 0 ? '' : hash.slice(at + 1)).get('out') ?? '';
  const steps: OutputStep[] = [];
  for (const entry of raw.split('|').filter(Boolean)) {
    const eq = entry.indexOf('=');
    const name = eq < 0 ? entry : entry.slice(0, eq);
    const rest = eq < 0 ? '' : entry.slice(eq + 1);
    const cut = rest.lastIndexOf(':');
    const pointer = cut < 0 ? rest : rest.slice(0, cut);
    const value = cut < 0 ? '' : rest.slice(cut + 1);
    if (name === 'ce') steps.push({ kind: 'ce' });
    else if (name === 'all' || name === 'reliable' || name === 'none')
      steps.push({ kind: 'mode', mode: name });
    else if (name === 'choice' && pointer && value) steps.push({ kind: 'choice', pointer, value });
    else if (name === 'number' && pointer && Number.isFinite(Number(value)) && value !== '')
      steps.push({ kind: 'number', pointer, value: Number(value) });
    else if (name === 'file' && Number.isFinite(Number(rest)) && rest !== '')
      steps.push({ kind: 'file', index: Number(rest) });
    else if (name === 'apply') steps.push({ kind: 'apply' });
    else if (name === 'confirm') steps.push({ kind: 'confirm' });
  }
  return steps;
}

/** Carry out the steps as soon as the draft and its first plan exist. Returns the stop function. */
export function runOutputLinks(
  store: OutputStore,
  editor: EditorStore,
  steps: readonly OutputStep[],
): () => void {
  if (steps.length === 0) return () => undefined;
  let next = 0;
  return effect(() => {
    void store.apply.value;
    const ready = editor.draft.value !== undefined && store.plan.value !== undefined;
    const settled = ready && !store.stale.value && !store.planning.value;
    if (!settled) return;
    while (next < steps.length) {
      const step = steps[next];
      if (!step) break;
      if (step.kind === 'apply' && !store.canApply.value) return;
      if (step.kind === 'confirm' && store.apply.value.phase !== 'confirm') return;
      next += 1;
      switch (step.kind) {
        case 'ce':
          store.setCeEnabled(true);
          return;
        case 'mode':
          store.setAcceptMode(step.mode);
          return;
        case 'choice':
          store.answer(step.pointer, step.value);
          return;
        case 'number':
          store.answer(step.pointer, sourced(step.value, 'answered'));
          return;
        case 'file':
          store.select(store.plan.value?.files[step.index]?.path ?? '');
          break;
        case 'apply':
          store.openApply();
          return;
        case 'confirm':
          void store.confirmApply({ backup: true, dryApply: true });
          return;
      }
    }
  });
}
