import { h } from 'preact';

interface Props {
  active?: boolean;
  icon: string;
  label: string;
  onClick: () => void;
  onHideTooltip: () => void;
  onShowTooltip: (label: string, element: HTMLElement) => void;
}

export function SidebarIconButton({ active = false, icon, label, onClick, onHideTooltip, onShowTooltip }: Props) {
  return (
    <div className="sidebar-icon-button-wrap">
      <button
        aria-label={label}
        className={`sidebar-icon-button ${active ? 'is-active' : ''}`}
        onBlur={onHideTooltip}
        onClick={onClick}
        onFocus={(event) => onShowTooltip(label, event.currentTarget)}
        onMouseEnter={(event) => onShowTooltip(label, event.currentTarget)}
        onMouseLeave={onHideTooltip}
        type="button"
      >
        <i className={`fa-solid ${icon}`} aria-hidden="true" />
      </button>
    </div>
  );
}
