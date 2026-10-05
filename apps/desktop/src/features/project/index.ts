import { lazyPage } from '~/shared/lazy';

/** The lazy page of the project feature. */
export const ProjectPage = lazyPage(() => import('./ProjectPage'));

/** The project selector of the top bar; small, so it is not lazy. */
export { ProjectSelector } from './ProjectSelector';
