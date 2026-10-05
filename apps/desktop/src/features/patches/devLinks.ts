import type { ConvertCandidateDto } from 'rimstudio-ipc-types';
import { getCurrentProject, openProjectAt } from '~/shared/project';
import { setAnswer } from './answerStore';
import { applyState, openReview, runApply } from './applyStore';
import { checked, focused } from './scanStore';

/**
 * Development deep links of the page, read from the hash query. Used for screenshots; production builds
 * ignore them.
 *
 *   #/patches?open=PATH     open the mod folder PATH as the project
 *   #/patches?view=lint     start on the lint view
 *   #/patches?focus=DEF     focus one weapon
 *   #/patches?tab=plan      start on a tab of the weapon (questions, plan, definition)
 *   #/patches?fill=1        answer every question of the focused weapon with sample values (fill=all: every weapon)
 *   #/patches?check=all     check every weapon that can be converted
 *   #/patches?review=1      open the apply review for the checked weapons (review=run also applies them)
 */
export function devLink(
  name: 'open' | 'view' | 'focus' | 'tab' | 'fill' | 'check' | 'review',
): string | undefined {
  if (!import.meta.env.DEV) return undefined;
  const query = window.location.hash.split('?')[1];
  return query ? (new URLSearchParams(query).get(name) ?? undefined) : undefined;
}

/** Open the project named by the open link, once. */
export async function openFromLink(): Promise<void> {
  const path = devLink('open');
  if (path) await openProjectAt(path).catch(() => undefined);
}

let linksApplied = false;

/** Apply the focus, fill, check and review links to the first scan of the page. */
export function applyScanLinks(list: readonly ConvertCandidateDto[]): void {
  if (linksApplied || !import.meta.env.DEV) return;
  linksApplied = true;
  const focus = devLink('focus');
  if (focus && list.some((c) => c.defName === focus)) focused.value = focus;
  if (devLink('check') === 'all') {
    checked.value = new Set(list.filter((c) => c.status === 'not-converted').map((c) => c.defName));
  }
  const fill = devLink('fill');
  if (fill) {
    const targets =
      fill === 'all' ? list : list.filter((c) => c.defName === (focused.peek() ?? ''));
    for (const target of targets) {
      for (const ask of target.asks) {
        const value =
          ask.field === '/ce/ammoSet'
            ? 'AmmoSet_303British'
            : ask.field === '/ce/weaponTagClass'
              ? 'CE_AI_SR'
              : ask.kind === 'flag'
                ? false
                : ask.suggestion;
        setAnswer(target, 'weapon', ask, value);
      }
    }
  }
  const project = getCurrentProject();
  if (devLink('review') && project) {
    const picked = list.filter((c) => checked.peek().has(c.defName));
    if (picked.length > 0) {
      void openReview(project, picked).then(() => {
        const state = applyState.peek();
        if (devLink('review') === 'run' && state.phase === 'review') {
          void runApply(project, state.items, { backup: true, dryApply: true });
        }
      });
    }
  }
}
