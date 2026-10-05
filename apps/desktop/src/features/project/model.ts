import type { IconName } from 'rimstudio-ui';
import type { LayoutIssueDto, NodeRoleDto, SeverityDto, TreeNodeDto } from 'rimstudio-ipc-types';
import type { MessageKey } from '~/shared/i18n';

// Display helpers of the project feature: which icon and label a role gets, how to find a node,
// how a path is joined. No layout rule lives here: roles, counts and issues come from the backend.

/** The icon drawn for a folder of each role. */
export const ROLE_ICON: Record<NodeRoleDto, IconName> = {
  about: 'info',
  'load-folders': 'settings',
  'content-root': 'folder',
  defs: 'folder',
  'defs-weapons': 'crosshair',
  'defs-sounds': 'sound',
  patches: 'patch',
  'ce-compat': 'link',
  textures: 'image',
  sounds: 'sound',
  languages: 'file',
  assemblies: 'lock',
  source: 'folder',
  other: 'folder',
};

/** The label of each role. */
export const ROLE_LABEL: Record<NodeRoleDto, MessageKey> = {
  about: 'project.role.about',
  'load-folders': 'project.role.load-folders',
  'content-root': 'project.role.content-root',
  defs: 'project.role.defs',
  'defs-weapons': 'project.role.defs-weapons',
  'defs-sounds': 'project.role.defs-sounds',
  patches: 'project.role.patches',
  'ce-compat': 'project.role.ce-compat',
  textures: 'project.role.textures',
  sounds: 'project.role.sounds',
  languages: 'project.role.languages',
  assemblies: 'project.role.assemblies',
  source: 'project.role.source',
  other: 'project.role.other',
};

const IMAGE = new Set(['png', 'dds', 'jpg', 'jpeg', 'gif', 'bmp', 'psd', 'xcf', 'ai']);
const SOUND = new Set(['wav', 'ogg', 'mp3']);

/** The extension of a file name in lower case, or an empty string. */
export function extensionOf(name: string): string {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : '';
}

/** The icon of a tree node: the role icon for a folder, the kind of file for a file. */
export function iconOf(node: TreeNodeDto): IconName {
  if (node.kind === 'folder') return ROLE_ICON[node.role];
  const ext = extensionOf(node.name);
  if (ext === 'xml') return 'xml';
  if (IMAGE.has(ext)) return 'image';
  if (SOUND.has(ext)) return 'sound';
  return 'file';
}

/** True when a file may be opened in the viewer (the backend still decides about binary files). */
export function isViewable(node: TreeNodeDto | undefined): node is TreeNodeDto {
  return node !== undefined && node.kind === 'file';
}

/** The tree id of the root node: the root path is empty, and a tree id must not be. */
export const ROOT_ID = '.';

/** The tree id of a node. */
export function idOf(node: { path: string }): string {
  return node.path === '' ? ROOT_ID : node.path;
}

/** The node with a path, or undefined. The root has the empty path. */
export function findNode(root: TreeNodeDto, path: string): TreeNodeDto | undefined {
  if (root.path === path) return root;
  for (const child of root.children) {
    if (path === child.path || path.startsWith(`${child.path}/`)) {
      const hit = findNode(child, path);
      if (hit) return hit;
    }
  }
  return undefined;
}

/** The ids of the nodes that start open: the root and its folders that hold the project's content. */
export function initiallyOpen(root: TreeNodeDto): string[] {
  const open = [idOf(root)];
  for (const child of root.children) {
    if (child.kind === 'folder' && child.role !== 'source' && child.role !== 'other')
      open.push(idOf(child));
  }
  return open;
}

/** The folders above a path, outermost first, including the root. */
export function ancestorsOf(path: string): string[] {
  const parts = path.split('/').filter(Boolean);
  const out = [''];
  for (let i = 1; i < parts.length; i += 1) out.push(parts.slice(0, i).join('/'));
  return out;
}

const RANK: Record<SeverityDto, number> = { error: 3, warning: 2, info: 1, hint: 0 };

/** The most serious severity of a list, or undefined for an empty list. */
export function worstSeverity(
  issues: readonly { severity: SeverityDto }[],
): SeverityDto | undefined {
  let worst: SeverityDto | undefined;
  for (const issue of issues) {
    if (worst === undefined || RANK[issue.severity] > RANK[worst]) worst = issue.severity;
  }
  return worst;
}

/** The issues on a path or below it. The root path (empty) holds every issue. */
export function issuesUnder(issues: readonly LayoutIssueDto[], path: string): LayoutIssueDto[] {
  if (path === '') return [...issues];
  return issues.filter((i) => i.path === path || i.path.startsWith(`${path}/`));
}

/** Join a folder and a name with the separator the folder already uses. */
export function joinPath(parent: string, name: string): string {
  const sep = parent.includes('\\') && !parent.includes('/') ? '\\' : '/';
  const base = parent.endsWith('/') || parent.endsWith('\\') ? parent.slice(0, -1) : parent;
  return `${base}${sep}${name}`;
}

const FORBIDDEN = new Set('\\/:*?"<>|');

/** A folder name for a mod name: characters that no file system accepts are removed. */
export function folderNameOf(name: string): string {
  let kept = '';
  for (const ch of name) {
    if (!FORBIDDEN.has(ch) && ch.charCodeAt(0) >= 32) kept += ch;
  }
  return kept
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/[. ]+$/, '');
}

function slug(text: string): string {
  return text
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '');
}

/** A package id suggestion from the author and the mod name: `author.name`. Only a suggestion. */
export function suggestPackageId(author: string, name: string): string {
  const parts = [slug(author), slug(name)].filter((p) => p.length > 0);
  return parts.join('.');
}

/** The supported versions typed as "1.5, 1.6": the non empty entries. */
export function splitVersions(text: string): string[] {
  return text
    .split(/[\s,;]+/)
    .map((v) => v.trim())
    .filter((v) => v.length > 0);
}

/** Codes whose fix is offered by the fix plan although the check gives them no fix of their own. */
const PLANNED_WITHOUT_KIND = new Set(['layout.ce-legacy-folder']);

/** True when the fix plan may carry out something for this issue (moves, edits, a folder rename). */
export function hasPlannedFix(issue: LayoutIssueDto): boolean {
  return (
    issue.fix.kind === 'move-file' ||
    issue.fix.kind === 'edit-load-folders' ||
    PLANNED_WITHOUT_KIND.has(issue.code)
  );
}
