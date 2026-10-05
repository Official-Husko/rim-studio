import type { ComponentType } from 'preact';
import type { IconName } from 'rimstudio-ui';
import { SetupPage } from '~/features/setup';
import { ProjectPage } from '~/features/project';
import { WeaponsPage } from '~/features/weapons';
import { PatchesPage } from '~/features/patches';
import { lazyPage } from '~/shared/lazy';
import type { MessageKey } from '~/shared/i18n';
import type { RouteId } from './route';

export interface ToolDescriptor {
  id: RouteId;
  titleKey: MessageKey;
  icon: IconName;
  Page: ComponentType;
  /** Shown only in development builds. */
  dev?: boolean;
}

const GalleryPage = lazyPage(() => import('~/gallery'));

/** The tool table of the temporary UI, in rail order. */
export const TOOLS: readonly ToolDescriptor[] = [
  { id: 'setup', titleKey: 'nav.setup', icon: 'settings', Page: SetupPage },
  { id: 'project', titleKey: 'nav.project', icon: 'folder', Page: ProjectPage },
  { id: 'weapons', titleKey: 'nav.weapons', icon: 'crosshair', Page: WeaponsPage },
  { id: 'patches', titleKey: 'nav.patches', icon: 'patch', Page: PatchesPage },
  { id: 'gallery', titleKey: 'nav.gallery', icon: 'grid', Page: GalleryPage, dev: true },
];

/** The tools visible in this build. */
export function visibleTools(dev: boolean = import.meta.env.DEV): ToolDescriptor[] {
  return TOOLS.filter((tool) => !tool.dev || dev);
}
