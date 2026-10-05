import { lazyPage } from '~/shared/lazy';

/** The only public entry of the weapons feature: its lazy page. A later task replaces the placeholder. */
export const WeaponsPage = lazyPage(() => import('./WeaponsPage'));
