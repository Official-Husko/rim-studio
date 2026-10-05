import type { ArchetypeProposalDto, DraftDto, ProposedValueDto } from 'rimstudio-ipc-types';
import { t, type MessageKey } from '~/shared/i18n';
import { getAt } from '../../model/pointer';
import { ACCURACY_FIELDS, COMMON_FIELDS, RANGED_FIELDS, TOOL_FIELDS } from '../../model/fields';
import { groupOfField, type GroupId } from './wizard-model';

/** One number of the result step: what it is, its value and its reason. */
export interface ResultRow {
  key: string;
  label: string;
  value: ProposedValueDto;
  unit: string | undefined;
  /** The value of the proposal before the last change, when it differs. */
  was: number | undefined;
  /** The number the open draft holds for a field the user decided (retune only). */
  yours: number | undefined;
}

/** A titled group of rows. */
export interface ResultGroup {
  id: GroupId;
  rows: ResultRow[];
}

const GROUP_ORDER: readonly GroupId[] = ['damage', 'range', 'firing', 'attacks', 'weight'];

/** Units worth showing beside a number; the others are implied by the label. */
const SHOWN_UNITS = new Set(['s', 'tiles', 'kg', 'ticks']);

function unitText(unit: string | undefined): string | undefined {
  return unit !== undefined && SHOWN_UNITS.has(unit) ? unit : undefined;
}

const SCALAR_FIELDS = [...RANGED_FIELDS, ...ACCURACY_FIELDS, ...COMMON_FIELDS];

/** The label and unit of a proposed field, from the pointer of the draft. */
export function describeField(
  proposal: ArchetypeProposalDto,
  pointer: string,
): { label: string; unit: string | undefined } {
  const scalar = SCALAR_FIELDS.find((f) => f.pointer === pointer);
  if (scalar) return { label: t(scalar.label), unit: unitText(scalar.unit) };
  const match = /^\/tools\/(\d+)\/(\w+)$/.exec(pointer);
  if (match) {
    const tool = proposal.tools[Number(match[1])];
    const field = TOOL_FIELDS.find((f) => f.suffix === match[2]);
    if (tool && field) {
      return {
        label: sentence(`${tool.label} ${t(field.label).toLowerCase()}`),
        unit: unitText(field.unit),
      };
    }
  }
  return { label: pointer, unit: undefined };
}

function rowOf(
  proposal: ArchetypeProposalDto,
  value: ProposedValueDto,
  before: ArchetypeProposalDto | undefined,
  draft: DraftDto | undefined,
): ResultRow {
  const { label, unit } = describeField(proposal, value.field);
  const held = value.locked && draft ? getAt(draft.spec, value.field) : undefined;
  const yours =
    typeof held === 'object' && held !== null ? (held as { value?: unknown }).value : undefined;
  const old = allValues(before).find((v) => v.field === value.field);
  return {
    key: value.field,
    label,
    value,
    unit,
    was: old !== undefined && old.value !== value.value ? old.value : undefined,
    yours: typeof yours === 'number' ? yours : undefined,
  };
}

/** Every proposed number, the scalar fields first, then the tools. */
export function allValues(proposal: ArchetypeProposalDto | undefined): ProposedValueDto[] {
  if (!proposal) return [];
  const tools = proposal.tools.flatMap((tool) => [
    tool.power,
    tool.cooldown,
    ...(tool.armorPenetration ? [tool.armorPenetration] : []),
  ]);
  return [...proposal.values, ...tools];
}

/** The numbers of a proposal in the groups of the result step, empty groups left out. */
export function groupRows(
  proposal: ArchetypeProposalDto,
  before: ArchetypeProposalDto | undefined,
  draft?: DraftDto,
): ResultGroup[] {
  const rows = allValues(proposal).map((v) => ({
    id: groupOfField(v.field),
    row: rowOf(proposal, v, before, draft),
  }));
  return GROUP_ORDER.map((id) => ({
    id,
    rows: rows.filter((r) => r.id === id).map((r) => r.row),
  })).filter((g) => g.rows.length > 0);
}

const GROUP_TITLES: Record<GroupId, MessageKey> = {
  damage: 'designer.wizard.group.damage',
  range: 'designer.wizard.group.range',
  firing: 'designer.wizard.group.firing',
  attacks: 'designer.wizard.group.attacks',
  weight: 'designer.wizard.group.weight',
};

/** The title of a group. */
export function groupTitle(id: GroupId, melee: boolean): string {
  if (id === 'attacks' && !melee) return t('designer.wizard.group.bash');
  return t(GROUP_TITLES[id]);
}

/** The stats shown in the live summary, in order, by kind. */
const HEADLINE: Record<'ranged' | 'melee', readonly string[]> = {
  ranged: ['damage', 'range', 'dps', 'cooldown', 'mass'],
  melee: ['swing_damage', 'dps', 'fight_dps', 'mass'],
};

/** The headline numbers of a proposal: the fit report's value of each headline stat. */
export function headlineStats(
  proposal: ArchetypeProposalDto,
): Array<{ stat: string; value: number }> {
  const perStat = proposal.fit?.perStat ?? [];
  return HEADLINE[proposal.kind].flatMap((stat) => {
    const found = perStat.find((s) => s.stat === stat);
    return found ? [{ stat, value: found.value }] : [];
  });
}

/**
 * The reason of a number without the "name value:" lead in, which the row already shows. A reason
 * that does not start that way is returned as it is.
 */
export function reasonText(reason: string): string {
  return sentence(reason.replace(/^[a-z][a-z ]*? -?[0-9][0-9.]*: /, ''));
}

/** The text with its first letter in upper case. */
export function sentence(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
