import { useState } from 'preact/hooks';
import type { ItemKindDto, ReferenceItemDto, TechLevelDto } from 'rimstudio-ipc-types';
import {
  Banner,
  Button,
  EmptyState,
  Icon,
  Panel,
  SegmentedControl,
  Select,
  Spinner,
  TextField,
} from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';
import { statLabel } from '../model/labels';
import type { ReferenceStore } from '../reference-store';

export interface ReferenceBrowserProps {
  store: ReferenceStore;
  /** False without an open project: cloning needs a project to store the draft in. */
  canClone: boolean;
  /** False without an open draft: an anchor is added to the open draft. */
  canAnchor: boolean;
  onClone: (item: ReferenceItemDto) => void;
  onAnchor: (item: ReferenceItemDto) => void;
  onShowDefinition: (item: ReferenceItemDto) => void;
}

const SHOWN_STATS: Record<ItemKindDto, readonly string[]> = {
  ranged: ['damage', 'dps', 'range', 'mass'],
  melee: ['swing_damage', 'dps', 'fight_dps', 'mass'],
};

function tierLabel(tier: TechLevelDto | undefined): string {
  switch (tier) {
    case 'neolithic':
      return t('designer.tier.neolithic');
    case 'medieval':
      return t('designer.tier.medieval');
    case 'industrial':
      return t('designer.tier.industrial');
    case 'spacer':
      return t('designer.tier.spacer');
    case 'ultra':
      return t('designer.tier.ultra');
    case 'archotech':
      return t('designer.tier.archotech');
    default:
      return '-';
  }
}

function ReferenceCard({
  item,
  kind,
  props,
}: {
  item: ReferenceItemDto;
  kind: ItemKindDto;
  props: ReferenceBrowserProps;
}) {
  return (
    <li class="flex flex-col gap-2 border border-line bg-bg p-2" data-def={item.defName}>
      <div class="flex items-baseline justify-between gap-2">
        <span class="min-w-0 truncate text-body font-semibold text-fg">{item.label}</span>
        <span class="shrink-0 font-mono text-mono-small text-muted">
          {item.strength === undefined ? '-' : formatNumber(item.strength, 2)}
        </span>
      </div>
      <p class="font-mono text-mono-small text-faint">
        {item.defName} / {tierLabel(item.tier)} / {item.role ?? '-'}
      </p>
      <dl class="grid grid-cols-4 gap-2 text-small">
        {SHOWN_STATS[kind].map((stat) => (
          <div key={stat} class="flex min-w-0 flex-col">
            <dt class="truncate text-faint">{statLabel(stat)}</dt>
            <dd class="font-mono text-mono-small tabular-nums text-fg">
              {item.stats[stat] === undefined ? '-' : formatNumber(item.stats[stat] ?? 0, 2)}
            </dd>
          </div>
        ))}
      </dl>
      <div class="flex flex-wrap gap-1">
        <Button
          size="sm"
          icon="copy"
          disabled={!props.canClone}
          onClick={() => props.onClone(item)}
        >
          {t('designer.reference.clone')}
        </Button>
        <Button
          size="sm"
          icon="link"
          disabled={!props.canAnchor}
          onClick={() => props.onAnchor(item)}
        >
          {t('designer.reference.anchor')}
        </Button>
        <Button size="sm" variant="ghost" icon="xml" onClick={() => props.onShowDefinition(item)}>
          {t('designer.reference.definition')}
        </Button>
      </div>
    </li>
  );
}

/** The weapons of the install, sorted by strength: filter, compare, clone, anchor, read the real definition. */
export function ReferenceBrowser(props: ReferenceBrowserProps) {
  const { store } = props;
  const [strongestFirst, setStrongestFirst] = useState(false);
  const kind = store.kind.value;
  const error = store.error.value;
  const loaded = store.items.value;
  const items = strongestFirst ? [...loaded].reverse() : loaded;
  const total = store.list.value?.total ?? 0;
  const tiers: Array<[TechLevelDto, string]> = [
    ['neolithic', t('designer.tier.neolithic')],
    ['medieval', t('designer.tier.medieval')],
    ['industrial', t('designer.tier.industrial')],
    ['spacer', t('designer.tier.spacer')],
    ['ultra', t('designer.tier.ultra')],
    ['archotech', t('designer.tier.archotech')],
  ];
  return (
    <Panel
      title={t('designer.panel.reference')}
      actions={store.loading.value ? <Spinner size="sm" label={t('app.loading')} /> : null}
    >
      <div class="flex flex-col gap-3">
        <SegmentedControl
          label={t('designer.reference.kind')}
          value={kind}
          onValueChange={(value) => store.setKind(value === 'melee' ? 'melee' : 'ranged')}
          options={[
            { value: 'ranged', label: t('designer.kind.ranged') },
            { value: 'melee', label: t('designer.kind.melee') },
          ]}
        />
        <div class="grid grid-cols-2 gap-2">
          <Select
            aria-label={t('designer.reference.tier')}
            value={store.tier.value ?? ''}
            options={[
              { value: '', label: t('designer.reference.anyTier') },
              ...tiers.map(([value, label]) => ({ value, label })),
            ]}
            onValueChange={(value) =>
              store.setTier(value === '' ? undefined : (value as TechLevelDto))
            }
          />
          <Select
            aria-label={t('designer.reference.role')}
            value={store.role.value ?? ''}
            options={[
              { value: '', label: t('designer.reference.anyRole') },
              ...store.roles.value.map((r) => ({ value: r, label: r })),
            ]}
            onValueChange={(value) => store.setRole(value === '' ? undefined : value)}
          />
        </div>
        <TextField
          type="search"
          aria-label={t('designer.reference.search')}
          placeholder={t('designer.reference.search')}
          prefix={<Icon name="search" />}
          value={store.text.value}
          onValueChange={(value) => {
            store.text.value = value;
          }}
        />
        <div class="flex items-center justify-between gap-2 text-small text-muted">
          <span>
            {store.list.value === undefined && store.loading.value
              ? t('designer.reference.loading')
              : t('designer.reference.count', { shown: items.length, total })}
          </span>
          <Button size="sm" variant="ghost" onClick={() => setStrongestFirst(!strongestFirst)}>
            {strongestFirst
              ? t('designer.reference.weakestFirst')
              : t('designer.reference.strongestFirst')}
          </Button>
        </div>
        {error ? (
          <Banner
            tone="error"
            title={error.code}
            action={
              <Button size="sm" onClick={() => void store.load()}>
                {t('designer.retry')}
              </Button>
            }
          >
            {error.message}
          </Banner>
        ) : null}
        {!error && items.length === 0 && !store.loading.value ? (
          <EmptyState
            compact
            title={t('designer.reference.emptyTitle')}
            description={t('designer.reference.emptyBody')}
          />
        ) : (
          <ul class="flex flex-col gap-2" aria-label={t('designer.panel.reference')}>
            {items.map((item) => (
              <ReferenceCard key={item.defName} item={item} kind={kind} props={props} />
            ))}
          </ul>
        )}
      </div>
    </Panel>
  );
}
