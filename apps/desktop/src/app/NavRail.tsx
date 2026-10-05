import { Icon } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { navigate, route, type RouteId } from './route';
import type { ToolDescriptor } from './tools';

export interface NavRailProps {
  tools: readonly ToolDescriptor[];
  active?: RouteId;
}

/** The left tool rail: one icon and a condensed caption per tool, an accent bar on the active one. */
export function NavRail({ tools, active = route.value }: NavRailProps) {
  return (
    <nav
      aria-label={t('nav.label')}
      class="flex w-rail shrink-0 flex-col border-r border-line bg-surface"
    >
      {tools.map((tool) => {
        const current = tool.id === active;
        return (
          <a
            key={tool.id}
            href={`#/${tool.id}`}
            aria-current={current ? 'page' : undefined}
            onClick={(e) => {
              e.preventDefault();
              navigate(tool.id);
            }}
            data-focus-inset
            class={
              current
                ? 'flex h-14 flex-col items-center justify-center gap-0.5 border-l-2 border-accent bg-accent-tint text-fg'
                : 'flex h-14 flex-col items-center justify-center gap-0.5 border-l-2 border-transparent text-muted hover:bg-hover hover:text-fg'
            }
          >
            <Icon name={tool.icon} size={20} />
            <span class="font-display text-small">{t(tool.titleKey)}</span>
          </a>
        );
      })}
    </nav>
  );
}
