import type {
  CeAmmoCatalogDto,
  CeAmmoEntryDto,
  CeAmmoSuggestionDto,
  CustomAmmoDto,
  DraftDto,
  WritePlanDto,
} from 'rimstudio-ipc-types';
import { createMockTransport, loadFixture, type MockHandler } from 'rimstudio-testkit/mock';
import { setTransport } from '~/shared/ipc';

/** Install a mock transport; the ammo commands answer from the fixtures recorded from the real bridge. */
export function installAmmoTransport(handlers: Record<string, MockHandler> = {}) {
  const transport = createMockTransport({
    handlers: {
      designer_ce_ammo_catalog: () => catalogSlice(),
      designer_ce_ammo_suggest: (request) =>
        (request as { copyFrom?: string }).copyFrom
          ? loadFixture('ammo-suggest-copy')
          : loadFixture('ammo-suggest-fmj'),
      ...handlers,
    },
  });
  setTransport(transport);
  return transport;
}

/** The slice of the real catalogue (22 sets with their facets). */
export function catalogSlice(): CeAmmoCatalogDto {
  const slice = loadFixture<CeAmmoCatalogDto>('ammo-catalog-slice');
  // the recorded file holds a slice of the 307 sets; the counts say so
  return { ...slice, total: slice.entries.length, matching: slice.entries.length };
}

/** The recorded suggestion for a full metal jacket type. */
export function suggestionFmj(): CeAmmoSuggestionDto {
  return loadFixture<CeAmmoSuggestionDto>('ammo-suggest-fmj');
}

/** The recorded plan of a weapon with a custom caliber of two types. */
export function customPlan(): WritePlanDto {
  return loadFixture<WritePlanDto>('ammo-plan-custom');
}

/** The recorded draft that carries that custom caliber. */
export function customDraft(): DraftDto {
  return loadFixture<DraftDto>('ammo-draft-custom');
}

/** The custom caliber of the recorded draft. */
export function customAmmo(): CustomAmmoDto {
  const ammo = customDraft().spec.ce?.customAmmo;
  if (!ammo) throw new Error('the recorded draft has no custom ammo');
  return ammo;
}

/** `count` catalogue entries made from the recorded ones, with distinct def names, for list size tests. */
export function manyEntries(count: number): CeAmmoEntryDto[] {
  const base = catalogSlice().entries;
  return Array.from({ length: count }, (_, i) => {
    const source = base[i % base.length] as CeAmmoEntryDto;
    return { ...source, defName: `${source.defName}_${i}`, label: `${source.label} ${i}` };
  });
}

/** A catalogue answer that serves `entries` in pages of the size asked for, as the backend does. */
export function pagedCatalog(entries: readonly CeAmmoEntryDto[]): MockHandler {
  return (request) => {
    const { page = 0, pageSize = 200 } = request as { page?: number; pageSize?: number };
    const slice = catalogSlice();
    return {
      ...slice,
      total: entries.length,
      matching: entries.length,
      page,
      pageSize,
      entries: entries.slice(page * pageSize, (page + 1) * pageSize),
    };
  };
}

/** Wait for pending promises and timers of zero delay. */
export async function settle(): Promise<void> {
  for (let i = 0; i < 6; i += 1) await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}
