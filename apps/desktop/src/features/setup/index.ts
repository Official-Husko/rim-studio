import { lazyPage } from '~/shared/lazy';

/** The only public entry of the setup feature: its lazy page. */
export const SetupPage = lazyPage(() => import('./SetupPage'));
