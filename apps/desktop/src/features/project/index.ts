import { lazyPage } from '~/shared/lazy';

/** The only public entry of the project feature: its lazy page. A later task replaces the placeholder. */
export const ProjectPage = lazyPage(() => import('./ProjectPage'));
