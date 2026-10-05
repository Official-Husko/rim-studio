import { useState } from 'preact/hooks';
import {
  Badge,
  Table,
  Tree,
  type SortDirection,
  type TableColumn,
  type TreeNode,
} from 'rimstudio-ui';
import { Section } from '../Section';

interface Row {
  id: string;
  label: string;
  kind: string;
  dps: number;
  status: string;
}

const ROWS: Row[] = [
  { id: 'Gun_AssaultRifle', label: 'Assault rifle', kind: 'ranged', dps: 9.4, status: 'written' },
  { id: 'Gun_Revolver', label: 'Revolver', kind: 'ranged', dps: 6.1, status: 'draft' },
  {
    id: 'Gun_BoltActionRifle',
    label: 'Bolt-action rifle',
    kind: 'ranged',
    dps: 5.2,
    status: 'draft',
  },
  {
    id: 'MeleeWeapon_LongSword',
    label: 'Longsword',
    kind: 'melee',
    dps: 11.3,
    status: 'out of date',
  },
];

const TREE: TreeNode[] = [
  {
    id: 'mod',
    label: 'Plasma Carbine',
    icon: 'folder',
    children: [
      {
        id: 'about',
        label: 'About',
        icon: 'folder',
        children: [
          { id: 'about.xml', label: 'About.xml', icon: 'xml' },
          { id: 'preview', label: 'Preview.png', icon: 'image' },
        ],
      },
      {
        id: 'defs',
        label: 'Defs',
        icon: 'folder',
        role: 'defs',
        children: [{ id: 'gun', label: 'Gun_PlasmaCarbine.xml', icon: 'xml' }],
      },
      {
        id: 'patches',
        label: 'Patches',
        icon: 'folder',
        role: 'patches',
        children: [{ id: 'ce', label: 'CE_Gun_PlasmaCarbine.xml', icon: 'patch' }],
      },
      {
        id: 'sounds',
        label: 'Sounds',
        icon: 'folder',
        children: [{ id: 'shot', label: 'Shot.ogg', icon: 'sound' }],
      },
    ],
  },
];

/** Table and tree. */
export function DataSection() {
  const [sort, setSort] = useState<{ key: string; direction: SortDirection }>({
    key: 'label',
    direction: 'asc',
  });
  const [selected, setSelected] = useState<Set<string>>(new Set(['Gun_Revolver']));
  const [node, setNode] = useState<string>('gun');
  const columns: TableColumn<Row>[] = [
    { key: 'label', header: 'Weapon', render: (r) => r.label, sortable: true },
    { key: 'id', header: 'defName', render: (r) => r.id, mono: true },
    { key: 'kind', header: 'Kind', render: (r) => r.kind },
    {
      key: 'dps',
      header: 'DPS',
      render: (r) => r.dps.toFixed(1),
      sortable: true,
      align: 'right',
      mono: true,
    },
    {
      key: 'status',
      header: 'Status',
      render: (r) => (
        <Badge
          tone={r.status === 'written' ? 'success' : r.status === 'draft' ? 'neutral' : 'warning'}
        >
          {r.status}
        </Badge>
      ),
    },
  ];
  const rows = [...ROWS].sort((a, b) => {
    const dir = sort.direction === 'asc' ? 1 : -1;
    return sort.key === 'dps' ? (a.dps - b.dps) * dir : a.label.localeCompare(b.label) * dir;
  });
  return (
    <Section id="data" title="Table and tree">
      <div class="max-w-4xl border border-line bg-surface">
        <Table
          label="Weapon drafts"
          columns={columns}
          rows={rows}
          getKey={(r) => r.id}
          sort={sort}
          onSortChange={(key, direction) => setSort({ key, direction })}
          selection="multiple"
          selectedKeys={selected}
          onSelectionChange={setSelected}
        />
      </div>
      <div class="max-w-4xl border border-line bg-surface">
        <Table
          label="Dense and empty"
          columns={columns}
          rows={[]}
          getKey={(r) => r.id}
          dense
          emptyText="No drafts yet"
        />
      </div>
      <div class="max-w-sm border border-line bg-surface p-2">
        <Tree
          label="Project files"
          nodes={TREE}
          selectedId={node}
          onSelect={setNode}
          defaultExpanded={['mod', 'defs']}
        />
      </div>
    </Section>
  );
}
