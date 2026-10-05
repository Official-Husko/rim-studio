import { useState } from 'preact/hooks';
import { Tabs, type TabItem } from 'rimstudio-ui';
import type { TreeNodeDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { BasicsTab } from './basics/BasicsTab';
import { pendingChanges } from './basics/aboutStore';
import { LinkCard } from './components/LinkCard';
import { devLink } from './devLinks';
import { FoldersTab } from './folders/FoldersTab';
import { foldersPending } from './folders/folderStore';
import { LayoutTab } from './LayoutTab';
import { ProjectHeader } from './ProjectHeader';
import { Workbench, selectNode } from './Workbench';
import type { ProjectView } from './store';

type TabId = 'basics' | 'folders' | 'files' | 'layout' | 'game';
const TAB_IDS: readonly TabId[] = ['basics', 'folders', 'files', 'layout', 'game'];

function startTab(): TabId {
  const wanted = devLink('tab');
  if (wanted === 'guide') return 'layout';
  return TAB_IDS.find((id) => id === wanted) ?? 'basics';
}

export interface ModViewProps {
  view: ProjectView;
  refreshing: boolean;
  onRefresh: () => void;
  onOpen: () => void;
  onNew: () => void;
  onClose: () => void;
}

/** An open mod: its header and the tabs Basics, Versions and folders, Files, Layout and Test in game. */
export function ModView({ view, refreshing, onRefresh, onOpen, onNew, onClose }: ModViewProps) {
  const [tab, setTab] = useState<TabId>(startTab);
  const projectId = view.summary.projectId;
  const unsaved = pendingChanges.value.length;
  const folderEdits = foldersPending.value;

  const tabs: TabItem[] = [
    {
      id: 'basics',
      label: t('project.tab.basics'),
      ...(unsaved > 0 ? { badge: String(unsaved) } : {}),
    },
    {
      id: 'folders',
      label: t('project.tab.folders'),
      ...(folderEdits > 0 ? { badge: String(folderEdits) } : {}),
    },
    { id: 'files', label: t('project.tab.file') },
    {
      id: 'layout',
      label: t('project.tab.layout'),
      ...(view.check.issues.length > 0 ? { badge: String(view.check.issues.length) } : {}),
    },
    { id: 'game', label: t('project.tab.game') },
  ];

  const show = (node: TreeNodeDto): void => {
    selectNode(node);
    setTab('files');
  };

  return (
    <>
      <ProjectHeader
        view={view}
        refreshing={refreshing}
        onRefresh={onRefresh}
        onOpen={onOpen}
        onNew={onNew}
        onClose={onClose}
      />
      <div class="border border-line bg-surface">
        <Tabs
          tabs={tabs}
          value={tab}
          onValueChange={(id) => setTab(id as TabId)}
          label={t('project.tabs.label')}
        >
          {(id) =>
            id === 'basics' ? (
              <BasicsTab projectId={projectId} />
            ) : id === 'folders' ? (
              <FoldersTab projectId={projectId} />
            ) : id === 'layout' ? (
              <LayoutTab view={view} onShowNode={show} />
            ) : id === 'game' ? (
              <div class="p-4">
                <LinkCard
                  key={`link-${projectId}`}
                  projectId={projectId}
                  projectPath={view.summary.path}
                  projectName={view.summary.name}
                />
              </div>
            ) : (
              <Workbench key={`workbench-${projectId}`} view={view} />
            )
          }
        </Tabs>
      </div>
    </>
  );
}
