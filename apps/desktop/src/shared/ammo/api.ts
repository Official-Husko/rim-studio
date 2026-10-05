import type {
  CeAmmoCatalogDto,
  CeAmmoSuggestionDto,
  DesignerCeAmmoCatalogRequest,
  DesignerCeAmmoSuggestRequest,
} from 'rimstudio-ipc-types';
import { callCommand } from '~/shared/ipc';

// One function per command used by the ammo browser and the custom ammo window; no state and no rules.

/** Entries the backend returns in one page at most. */
export const CATALOG_PAGE = 200;

/** One page of the catalogue of every ammo set of the install. */
export function ammoCatalog(request: DesignerCeAmmoCatalogRequest): Promise<CeAmmoCatalogDto> {
  return callCommand('designer_ce_ammo_catalog', request);
}

/** A new ammo type filled from the nearest of the user's own ammunition, or copied from one. */
export function ammoSuggest(request: DesignerCeAmmoSuggestRequest): Promise<CeAmmoSuggestionDto> {
  return callCommand('designer_ce_ammo_suggest', request);
}
