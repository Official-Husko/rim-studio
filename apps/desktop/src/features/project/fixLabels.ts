import type { BadgeTone } from 'rimstudio-ui';
import type { LayoutFixItemDto, LayoutFixItemKindDto, LayoutFixRiskDto } from 'rimstudio-ipc-types';
import type { MessageKey } from '~/shared/i18n';

/** The label of each kind of plan item. */
export const FIX_KIND_LABEL: Record<LayoutFixItemKindDto, MessageKey> = {
  'move-file': 'project.fix.kind.move-file',
  'move-folder': 'project.fix.kind.move-folder',
  'create-folder': 'project.fix.kind.create-folder',
  'edit-load-folders': 'project.fix.kind.edit-load-folders',
  'create-load-folders': 'project.fix.kind.create-load-folders',
};

/** The label and the tone of each risk. */
export const FIX_RISK_LABEL: Record<LayoutFixRiskDto, MessageKey> = {
  safe: 'project.fix.risk.safe',
  'needs-review': 'project.fix.risk.needs-review',
};
export const FIX_RISK_TONE: Record<LayoutFixRiskDto, BadgeTone> = {
  safe: 'success',
  'needs-review': 'warning',
};

/** The sentence key that tells what an item does in the confirmation. */
export function confirmKey(kind: LayoutFixItemKindDto): MessageKey {
  switch (kind) {
    case 'move-file':
      return 'project.fix.confirm.move';
    case 'move-folder':
      return 'project.fix.confirm.rename';
    case 'create-folder':
    case 'create-load-folders':
      return 'project.fix.confirm.create';
    default:
      return 'project.fix.confirm.edit';
  }
}

/** The name an item gets in the lists: its destination, or the source when there is none. */
export function itemTarget(item: LayoutFixItemDto): string {
  return item.to || item.from;
}
