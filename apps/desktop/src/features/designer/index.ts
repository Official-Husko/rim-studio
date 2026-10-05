import { lazyPage } from '~/shared/lazy';

/** The only public entry of the designer feature: the Weapons page, loaded on first visit. */
export const DesignerPage = lazyPage(() => import('./DesignerPage'));
