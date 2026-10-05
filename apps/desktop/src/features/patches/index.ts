import { lazyPage } from '~/shared/lazy';

/** The only public entry of the patches feature: its lazy page. A later task replaces the placeholder. */
export const PatchesPage = lazyPage(() => import('./PatchesPage'));
