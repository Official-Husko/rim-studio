import { useEffect, useState } from 'preact/hooks';
import { SplitPane, Tabs, type TabItem } from 'rimstudio-ui';
import type { TreeNodeDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { devLink } from './devLinks';
import { FileViewer } from './FileViewer';
import { FolderSummary } from './FolderSummary';
import { LayoutGuide } from './LayoutGuide';
import { LayoutPanel } from './LayoutPanel';
import { ProjectTree } from './ProjectTree';
import { findNode, issuesUnder } from './model';
import {
  clearFile,
  fileView,
  fixError,
  fixMissingFolders,
  fixResult,
  fixing,
  selectedPath,
  showFile,
  type ProjectView,
} from './store';

type TabId = 'file' | 'layout' | 'guide';

/** The tree on the left; the file viewer, the layout check and the layout guide on the right. */
export function Workbench({ view }: { view: ProjectView }) {
  const [tab, setTab] = useState<TabId>(() => {
    const wanted = devLink('tab');
    return wanted === 'layout' || wanted === 'guide' ? wanted : 'file';
  });
  const { tree, check } = view;
  const path = selectedPath.value;
  const node = path === undefined ? undefined : findNode(tree.root, path);

  useEffect(() => {
    const file = devLink('file');
    if (file) void showFile(file);
  }, []);

  const select = (picked: TreeNodeDto): void => {
    setTab('file');
    if (picked.kind === 'file') void showFile(picked.path);
    else {
      clearFile();
      selectedPath.value = picked.path;
    }
  };

  const tabs: TabItem[] = [
    { id: 'file', label: t('project.tab.file') },
    {
      id: 'layout',
      label: t('project.tab.layout'),
      ...(check.issues.length > 0 ? { badge: String(check.issues.length) } : {}),
    },
    { id: 'guide', label: t('project.tab.guide') },
  ];

  return (
    <div class="min-h-96 flex-1 border border-line bg-surface">
      <SplitPane
        label={t('project.split.label')}
        defaultSize={420}
        min={240}
        first={<ProjectTree tree={tree} selectedPath={path} onSelect={select} />}
        second={
          <Tabs
            tabs={tabs}
            value={tab}
            onValueChange={(id) => setTab(id as TabId)}
            label={t('project.tabs.label')}
          >
            {(id) =>
              id === 'layout' ? (
                <LayoutPanel
                  check={check}
                  fixing={fixing.value}
                  fixResult={fixResult.value}
                  fixError={fixError.value}
                  onFix={() => void fixMissingFolders()}
                  onShowPath={(target) => {
                    const hit = findNode(tree.root, target);
                    if (hit) select(hit);
                  }}
                />
              ) : id === 'guide' ? (
                <LayoutGuide />
              ) : node && node.kind === 'folder' ? (
                <FolderSummary node={node} issues={issuesUnder(tree.issues, node.path)} />
              ) : (
                <FileViewer file={fileView.value} />
              )
            }
          </Tabs>
        }
      />
    </div>
  );
}
