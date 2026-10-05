import type {
  ArchetypeCatalogDto,
  ArchetypeDto,
  ArchetypeModeDto,
  BalanceTargetDto,
  DescriptorsDto,
  ItemKindDto,
  RateOfFireDto,
  TechLevelDto,
} from 'rimstudio-ipc-types';

/** The steps of the wizard, in order. */
export const WIZARD_STEPS = ['category', 'describe', 'result', 'name'] as const;
export type WizardStep = (typeof WIZARD_STEPS)[number];

/** The descriptors an archetype can list in `applies`. */
export type DescriptorId = 'action' | 'rof' | 'calibre' | 'handling' | 'tier' | 'balance';

/** What the user has chosen so far. */
export interface WizardChoice {
  archetypeId: string | undefined;
  descriptors: DescriptorsDto;
  balance: BalanceTargetDto;
  /** Use a Combat Extended ammo set as the calibre (proposal in Combat Extended mode). */
  ceCalibre: boolean;
}

/** The choice before anything is picked. */
export function emptyChoice(): WizardChoice {
  return { archetypeId: undefined, descriptors: {}, balance: 'typical', ceCalibre: false };
}

/** Changes to the descriptors; an undefined entry removes the descriptor. */
export type DescriptorPatch = { [K in keyof DescriptorsDto]?: DescriptorsDto[K] | undefined };

/** The descriptors with a patch applied. */
export function mergeDescriptors(base: DescriptorsDto, patch: DescriptorPatch): DescriptorsDto {
  const merged: Record<string, unknown> = { ...base, ...patch };
  for (const key of Object.keys(merged)) {
    if (merged[key] === undefined) delete merged[key];
  }
  // the keys are the ones of DescriptorsDto: both inputs are
  return merged as DescriptorsDto;
}

/** The mode of the proposal request for a choice. */
export function modeOf(choice: WizardChoice): ArchetypeModeDto {
  return choice.ceCalibre && choice.descriptors.ammoSet ? 'combat-extended' : 'vanilla';
}

/** True when the archetype lists the descriptor as one the user can set. */
export function applies(archetype: ArchetypeDto | undefined, id: DescriptorId): boolean {
  return archetype?.applies.includes(id) ?? false;
}

/** The descriptors a fresh choice of the archetype starts with: its own defaults. */
export function defaultDescriptors(archetype: ArchetypeDto): DescriptorsDto {
  const out: DescriptorsDto = {
    rof: { class: archetype.defaultRof },
    handling: archetype.defaultHandling,
  };
  if (archetype.defaultAction) out.action = archetype.defaultAction;
  if (archetype.defaultCalibre) out.calibre = archetype.defaultCalibre;
  return out;
}

/** The archetype with this id, searched through every family. */
export function findArchetype(
  catalog: ArchetypeCatalogDto | undefined,
  id: string | undefined,
): ArchetypeDto | undefined {
  if (!catalog || !id) return undefined;
  for (const family of catalog.families) {
    const found = family.archetypes.find((a) => a.id === id);
    if (found) return found;
  }
  return undefined;
}

/** The families of one kind of weapon. */
export function familiesOf(catalog: ArchetypeCatalogDto | undefined, kind: ItemKindDto) {
  return (catalog?.families ?? []).filter((f) => f.kind === kind);
}

/** The rounds per minute the rate of fire stands for, for a gun with a typical rate. */
export function rpmOf(
  archetype: ArchetypeDto,
  catalog: ArchetypeCatalogDto,
  rof: RateOfFireDto | undefined,
): number | undefined {
  const ref = archetype.refRpm;
  if (ref === undefined) return undefined;
  if (rof && 'rpm' in rof) return rof.rpm;
  const rate = catalog.rof.find((r) => r.id === (rof?.class ?? archetype.defaultRof))?.rate ?? 1;
  return ref * rate;
}

/** The scale of the rate of fire slider: from well under slow to well over fast. */
export function rpmRange(
  archetype: ArchetypeDto,
  catalog: ArchetypeCatalogDto,
): { min: number; max: number; step: number } | undefined {
  const ref = archetype.refRpm;
  if (ref === undefined) return undefined;
  const rates = catalog.rof.map((r) => r.rate ?? 1);
  const step = ref >= 200 ? 10 : ref >= 40 ? 5 : ref >= 10 ? 1 : 0.5;
  const min = Math.max(step, Math.ceil((ref * Math.min(...rates) * 0.8) / step) * step);
  const max = Math.floor((ref * Math.max(...rates) * 1.25) / step) * step;
  return { min, max, step };
}

/** Seconds between two shots at a rate of fire in rounds per minute. */
export function secondsBetweenShots(rpm: number): number {
  return rpm > 0 ? 60 / rpm : 0;
}

/** The label of a tier with the number of reference weapons the install has there. */
export function tierCount(
  catalog: ArchetypeCatalogDto,
  tier: TechLevelDto,
  kind: ItemKindDto,
): number {
  const row = catalog.tiers.find((x) => x.tier === tier);
  return kind === 'ranged' ? (row?.rangedCount ?? 0) : (row?.meleeCount ?? 0);
}

/** The groups of the result step, in display order. */
export type GroupId = 'damage' | 'range' | 'firing' | 'attacks' | 'weight';

/** The group a proposed field belongs to. */
export function groupOfField(pointer: string): GroupId {
  if (pointer.startsWith('/tools/')) return 'attacks';
  if (pointer === '/ranged/damage' || pointer === '/ranged/armorPenetration') return 'damage';
  if (pointer === '/ranged/range' || pointer.startsWith('/ranged/accuracy/')) return 'range';
  if (pointer.startsWith('/ranged/')) return 'firing';
  return 'weight';
}

/** A def name from a prefix and a label: the words joined without spaces or punctuation. */
export function suggestDefName(prefix: string, label: string): string {
  const words = label.match(/[A-Za-z0-9]+/g) ?? [];
  const body = words.map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join('');
  return body === '' ? '' : `${prefix.trim()}${body}`;
}

/** The resolved descriptors of a proposal as plain label and text pairs, for the summaries. */
export function describeResolved(
  catalog: ArchetypeCatalogDto,
  archetype: ArchetypeDto,
  resolved: {
    action?: string | undefined;
    rof: string;
    calibre?: string | undefined;
    ammoSet?: string | undefined;
    handling: string;
    tier: TechLevelDto;
  },
  ammoText?: string,
): Array<{ key: 'action' | 'rof' | 'calibre' | 'handling' | 'tier'; text: string }> {
  const out: Array<{ key: 'action' | 'rof' | 'calibre' | 'handling' | 'tier'; text: string }> = [];
  const action = catalog.actions.find((a) => a.id === resolved.action)?.label;
  if (action) out.push({ key: 'action', text: action });
  const rof = catalog.rof.find((r) => r.id === resolved.rof)?.label ?? resolved.rof;
  out.push({ key: 'rof', text: rof });
  if (resolved.ammoSet) out.push({ key: 'calibre', text: ammoText ?? resolved.ammoSet });
  else if (resolved.calibre) {
    out.push({
      key: 'calibre',
      text: catalog.calibres.find((c) => c.id === resolved.calibre)?.label ?? resolved.calibre,
    });
  }
  out.push({
    key: 'handling',
    text: catalog.handlings.find((h) => h.id === resolved.handling)?.label ?? resolved.handling,
  });
  out.push({
    key: 'tier',
    text: catalog.tiers.find((x) => x.tier === resolved.tier)?.label ?? archetype.defaultTier,
  });
  return out;
}

/** The label of a Combat Extended ammo set the wizard has loaded, when it knows it. */
export function ammoSetLabel(
  store: { calibres: { value: Array<{ set: string; label: string }> | undefined } },
  set: string | undefined,
): string | undefined {
  return set === undefined ? undefined : store.calibres.value?.find((c) => c.set === set)?.label;
}
