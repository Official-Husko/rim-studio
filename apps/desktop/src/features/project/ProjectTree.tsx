import { useEffect, useMemo, useState } from 'preact/hooks';
import { Badge, Tree, type BadgeTone, type TreeNode } from 'rimstudio-ui';
import type { LayoutIssueDto, ProjectTreeDto, SeverityDto, TreeNodeDto } from 'rimstudio-ipc-types';
import { formatBytes, formatNumber } from '~/shared/format';
import { t, tn } from '~/shared/i18n';
import {
  ROLE_LABEL,
  ROOT_ID,
  ancestorsOf,
  iconOf,
  idOf,
  initiallyOpen,
  issuesUnder,
  worstSeverity,
} from './model';

const TONES: Record<SeverityDto, BadgeTone> = {
  error: 'danger',
  warning: 'warning',
  info: 'info',
  hint: 'neutral',
};

/** Roles whose folders get a badge; a plain folder or file shows none. */
function showsRole(node: TreeNodeDto, parent: TreeNodeDto | undefined): boolean {
  if (node.role === 'other') return false;
  if (node.kind === 'file') return false;
  // a folder named like its role (Patches, Textures) needs no badge saying so
  if (node.name.toLowerCase() === t(ROLE_LABEL[node.role]).toLowerCase()) return false;
  // a child with the role of its parent (Defs below Defs) would repeat the same badge
  return parent === undefined || parent.role !== node.role;
}

function nodeOf(
  node: TreeNodeDto,
  parent: TreeNodeDto | undefined,
  issues: readonly LayoutIssueDto[],
): TreeNode {
  const here = node.issues > 0 ? issuesUnder(issues, node.path) : [];
  const worst = worstSeverity(here);
  const trailing = (
    <>
      {worst ? <Badge tone={TONES[worst]}>{tn('project.tree.issues', node.issues)}</Badge> : null}
      {showsRole(node, parent) ? <Badge tone="info">{t(ROLE_LABEL[node.role])}</Badge> : null}
      <span class="font-mono text-mono-small text-faint">
        {node.kind === 'folder'
          ? `${formatNumber(node.files, 0)} / ${formatBytes(node.bytes)}`
          : formatBytes(node.bytes)}
      </span>
    </>
  );
  return {
    id: idOf(node),
    label: node.name,
    icon: iconOf(node),
    trailing,
    title: node.path === '' ? node.name : node.path,
    children: node.children.map((child) => nodeOf(child, node, issues)),
  };
}

export interface ProjectTreeProps {
  tree: ProjectTreeDto;
  selectedPath: string | undefined;
  onSelect: (node: TreeNodeDto) => void;
}

function indexOf(node: TreeNodeDto, into: Map<string, TreeNodeDto>): void {
  into.set(idOf(node), node);
  for (const child of node.children) indexOf(child, into);
}

/** The annotated folder tree: role badges on folders, file counts and sizes, issue markers. */
export function ProjectTree({ tree, selectedPath, onSelect }: ProjectTreeProps) {
  const nodes = useMemo(() => [nodeOf(tree.root, undefined, tree.issues)], [tree]);
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set(initiallyOpen(tree.root)));
  // a file chosen elsewhere (an issue, a link) opens the folders above it
  useEffect(() => {
    if (selectedPath === undefined) return;
    setExpanded((now) => {
      const next = new Set(now);
      for (const folder of ancestorsOf(selectedPath)) next.add(folder === '' ? ROOT_ID : folder);
      return next.size === now.size ? now : next;
    });
  }, [selectedPath]);
  const byPath = useMemo(() => {
    const map = new Map<string, TreeNodeDto>();
    indexOf(tree.root, map);
    return map;
  }, [tree]);
  return (
    <div class="flex min-h-0 flex-col">
      {tree.truncated ? (
        <p class="m-0 px-3 py-2 text-small text-warning">{t('project.tree.truncated')}</p>
      ) : null}
      <Tree
        label={t('project.tree.label')}
        nodes={nodes}
        selectedId={
          selectedPath === undefined ? undefined : selectedPath === '' ? ROOT_ID : selectedPath
        }
        expanded={expanded}
        onExpandedChange={setExpanded}
        onSelect={(id) => {
          const hit = byPath.get(id);
          if (hit) onSelect(hit);
        }}
      />
    </div>
  );
}
