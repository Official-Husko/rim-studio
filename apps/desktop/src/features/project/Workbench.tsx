import { useEffect } from 'preact/hooks';
import { SplitPane } from 'rimstudio-ui';
import type { TreeNodeDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { devLink } from './devLinks';
import { FileViewer } from './FileViewer';
import { FolderSummary } from './FolderSummary';
import { ProjectTree } from './ProjectTree';
import { findNode, issuesUnder } from './model';
import { clearFile, fileView, selectedPath, showFile, type ProjectView } from './store';

/** Choose a node of the tree: a file opens in the viewer, a folder shows its summary. */
export function selectNode(node: TreeNodeDto): void {
  if (node.kind === 'file') void showFile(node.path);
  else {
    clearFile();
    selectedPath.value = node.path;
  }
}

/** Files: the annotated tree on the left, the file viewer or the folder summary on the right. */
export function Workbench({ view }: { view: ProjectView }) {
  const { tree } = view;
  const path = selectedPath.value;
  const node = path === undefined ? undefined : findNode(tree.root, path);

  useEffect(() => {
    const file = devLink('file');
    if (file) void showFile(file);
  }, []);

  return (
    <div class="h-160 border-t border-line bg-surface">
      <SplitPane
        label={t('project.split.label')}
        defaultSize={420}
        min={240}
        first={<ProjectTree tree={tree} selectedPath={path} onSelect={selectNode} />}
        second={
          node && node.kind === 'folder' ? (
            <FolderSummary node={node} issues={issuesUnder(tree.issues, node.path)} />
          ) : (
            <FileViewer file={fileView.value} />
          )
        }
      />
    </div>
  );
}
