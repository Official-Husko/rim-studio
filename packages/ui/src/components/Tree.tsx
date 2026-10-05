import { useMemo, useRef, useState } from 'preact/hooks';
import { cx } from '../cx';
import { Icon, type IconName } from './Icon';

export interface TreeNode {
  id: string;
  label: string;
  icon?: IconName;
  /** A short role text drawn as a mono tag after the label (a def type, a folder role). */
  role?: string;
  children?: TreeNode[];
}

export interface TreeProps {
  nodes: TreeNode[];
  /** Accessible name of the tree. */
  label: string;
  selectedId?: string;
  onSelect?: (id: string) => void;
  /** Controlled expansion; leave undefined to let the tree keep its own. */
  expanded?: ReadonlySet<string>;
  onExpandedChange?: (expanded: Set<string>) => void;
  defaultExpanded?: string[];
}

interface Flat {
  node: TreeNode;
  level: number;
  parentId?: string;
  index: number;
  count: number;
}

function flatten(
  nodes: TreeNode[],
  expanded: ReadonlySet<string>,
  level = 1,
  parentId?: string,
  out: Flat[] = [],
): Flat[] {
  nodes.forEach((node, index) => {
    out.push({ node, level, parentId, index: index + 1, count: nodes.length });
    if (node.children?.length && expanded.has(node.id))
      flatten(node.children, expanded, level + 1, node.id, out);
  });
  return out;
}

/** An expandable tree with the standard keyboard model (arrows, Home, End, Enter, Space). */
export function Tree({
  nodes,
  label,
  selectedId,
  onSelect,
  expanded: controlled,
  onExpandedChange,
  defaultExpanded,
}: TreeProps) {
  const [own, setOwn] = useState<Set<string>>(new Set(defaultExpanded ?? []));
  const expanded = controlled ?? own;
  const flat = useMemo(() => flatten(nodes, expanded), [nodes, expanded]);
  const [focusId, setFocusId] = useState<string | undefined>(selectedId);
  const refs = useRef(new Map<string, HTMLDivElement>());
  const current =
    flat.find((f) => f.node.id === focusId) ??
    flat.find((f) => f.node.id === selectedId) ??
    flat[0];

  const setExpanded = (next: Set<string>): void => {
    setOwn(next);
    onExpandedChange?.(next);
  };
  const toggle = (id: string, open: boolean): void => {
    const next = new Set(expanded);
    if (open) next.add(id);
    else next.delete(id);
    setExpanded(next);
  };
  const focus = (id: string | undefined): void => {
    if (!id) return;
    setFocusId(id);
    refs.current.get(id)?.focus();
  };

  const onKeyDown = (event: KeyboardEvent, item: Flat): void => {
    const pos = flat.indexOf(item);
    const hasChildren = Boolean(item.node.children?.length);
    const open = expanded.has(item.node.id);
    switch (event.key) {
      case 'ArrowDown':
        focus(flat[pos + 1]?.node.id);
        break;
      case 'ArrowUp':
        focus(flat[pos - 1]?.node.id);
        break;
      case 'Home':
        focus(flat[0]?.node.id);
        break;
      case 'End':
        focus(flat[flat.length - 1]?.node.id);
        break;
      case 'ArrowRight':
        if (hasChildren && !open) toggle(item.node.id, true);
        else if (hasChildren) focus(flat[pos + 1]?.node.id);
        break;
      case 'ArrowLeft':
        if (hasChildren && open) toggle(item.node.id, false);
        else focus(item.parentId);
        break;
      case 'Enter':
      case ' ':
        onSelect?.(item.node.id);
        break;
      default:
        return;
    }
    event.preventDefault();
  };

  return (
    <div role="tree" aria-label={label} class="text-body">
      {flat.map((item) => {
        const { node } = item;
        const hasChildren = Boolean(node.children?.length);
        const open = expanded.has(node.id);
        const selected = node.id === selectedId;
        return (
          <div
            key={node.id}
            ref={(el) => {
              if (el) refs.current.set(node.id, el);
              else refs.current.delete(node.id);
            }}
            role="treeitem"
            aria-level={item.level}
            aria-posinset={item.index}
            aria-setsize={item.count}
            aria-expanded={hasChildren ? (open ? 'true' : 'false') : undefined}
            aria-selected={selected ? 'true' : 'false'}
            tabIndex={current?.node.id === node.id ? 0 : -1}
            style={`--lvl:${item.level - 1}`}
            onFocus={() => setFocusId(node.id)}
            onClick={() => onSelect?.(node.id)}
            onKeyDown={(e) => onKeyDown(e, item)}
            class={cx(
              'flex h-row cursor-default items-center gap-1 pr-2 pl-[calc(var(--lvl)*4*var(--rs-space)+var(--rs-space))] hover:bg-hover',
              selected && 'bg-accent-tint font-semibold',
            )}
          >
            {hasChildren ? (
              <button
                type="button"
                tabIndex={-1}
                aria-hidden="true"
                onClick={(e) => {
                  e.stopPropagation();
                  toggle(node.id, !open);
                }}
                class="inline-flex size-4 items-center justify-center text-muted"
              >
                <Icon name="chevron" turn={open ? 1 : 0} />
              </button>
            ) : (
              <span class="size-4" />
            )}
            {node.icon ? (
              <span class="text-muted">
                <Icon name={node.icon} />
              </span>
            ) : null}
            <span class="min-w-0 flex-1 truncate">{node.label}</span>
            {node.role ? (
              <span class="font-mono text-mono-small text-faint">{node.role}</span>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}
