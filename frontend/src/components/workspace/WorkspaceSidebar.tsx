import { h } from 'preact';
import { useEffect, useRef, useState } from 'preact/hooks';
import { workspaceTabs } from '../../constants';
import type { WorkspaceTab } from '../../types';
import { SidebarIconButton } from './SidebarIconButton';

interface Props {
  activeTab: WorkspaceTab;
  onTabChange: (tab: WorkspaceTab) => void;
}

export function WorkspaceSidebar({ activeTab, onTabChange }: Props) {
  const [tooltip, setTooltip] = useState<{ label: string; top: number; left: number } | null>(null);
  const tooltipTimeoutRef = useRef<number | null>(null);
  const settingsTab = workspaceTabs.find((tab) => tab.key === 'settings');
  const primaryTabs = workspaceTabs.filter((tab) => tab.key !== 'settings');

  useEffect(() => {
    function hideTooltipOnViewportChange() {
      setTooltip(null);
    }

    window.addEventListener('scroll', hideTooltipOnViewportChange, true);
    window.addEventListener('resize', hideTooltipOnViewportChange);

    return () => {
      if (tooltipTimeoutRef.current !== null) {
        window.clearTimeout(tooltipTimeoutRef.current);
      }
      window.removeEventListener('scroll', hideTooltipOnViewportChange, true);
      window.removeEventListener('resize', hideTooltipOnViewportChange);
    };
  }, []);

  useEffect(() => {
    if (tooltipTimeoutRef.current !== null) {
      window.clearTimeout(tooltipTimeoutRef.current);
      tooltipTimeoutRef.current = null;
    }

    if (!tooltip) {
      return;
    }

    tooltipTimeoutRef.current = window.setTimeout(() => {
      setTooltip(null);
      tooltipTimeoutRef.current = null;
    }, 5000); // Auto-hide tooltip after 5 seconds
  }, [tooltip]);

  function showTooltip(label: string, element: HTMLElement) {
    const rect = element.getBoundingClientRect();
    setTooltip({
      label,
      top: rect.top + rect.height / 2,
      left: rect.right + 12,
    });
  }

  function hideTooltip() {
    if (tooltipTimeoutRef.current !== null) {
      window.clearTimeout(tooltipTimeoutRef.current);
      tooltipTimeoutRef.current = null;
    }
    setTooltip(null);
  }

  return (
    <div className="workspace-sidebar-slot">
      <aside className="workspace-sidebar">
        <div className="workspace-sidebar-top">
          <nav className="tab-list sidebar-scroll-list" aria-label="Workspace Sections">
            {primaryTabs.map((tab) => (
              <SidebarIconButton
                active={activeTab === tab.key}
                icon={tab.icon}
                key={tab.key}
                label={tab.label}
                onHideTooltip={hideTooltip}
                onClick={() => onTabChange(tab.key)}
                onShowTooltip={showTooltip}
              />
            ))}
          </nav>
        </div>

        {settingsTab ? (
          <div className="sidebar-sticky-segment">
            <SidebarIconButton
              active={activeTab === settingsTab.key}
              icon={settingsTab.icon}
              label={settingsTab.label}
              onHideTooltip={hideTooltip}
              onClick={() => onTabChange(settingsTab.key)}
              onShowTooltip={showTooltip}
            />
          </div>
        ) : null}
      </aside>

      {tooltip ? (
        <div className="sidebar-floating-tooltip" role="tooltip" style={{ left: `${tooltip.left}px`, top: `${tooltip.top}px` }}>
          {tooltip.label}
        </div>
      ) : null}
    </div>
  );
}
