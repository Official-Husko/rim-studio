import { Panel } from 'rimstudio-ui';
import type { TreeNodeDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { LayoutFacts } from './LayoutFacts';
import { LayoutGuide } from './LayoutGuide';
import { LayoutPanel } from './LayoutPanel';
import { findNode } from './model';
import { fixError, fixMissingFolders, fixResult, fixing, type ProjectView } from './store';

export interface LayoutTabProps {
  view: ProjectView;
  /** Show a path in the Files tab. */
  onShowNode: (node: TreeNodeDto) => void;
}

/** The layout check with its fixes, how the mod is laid out, and the guide to the layout. */
export function LayoutTab({ view, onShowNode }: LayoutTabProps) {
  return (
    <div class="flex flex-col gap-4 p-4">
      <LayoutFacts view={view} />
      <Panel title={t('project.layout.check')} framed>
        <LayoutPanel
          check={view.check}
          fixing={fixing.value}
          fixResult={fixResult.value}
          fixError={fixError.value}
          onFix={() => void fixMissingFolders()}
          onShowPath={(target) => {
            const hit = findNode(view.tree.root, target);
            if (hit) onShowNode(hit);
          }}
        />
      </Panel>
      <Panel title={t('project.tab.guide')} framed collapsible defaultCollapsed>
        <LayoutGuide />
      </Panel>
    </div>
  );
}
