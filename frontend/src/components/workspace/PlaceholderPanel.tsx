import { h } from 'preact';

interface Props {
  icon?: string;
}

export function PlaceholderPanel({ icon = 'fa-person-digging' }: Props) {
  return (
    <section className="panel-card placeholder-panel coming-soon-panel">
      <i className={`fa-solid ${icon} coming-soon-icon`} aria-hidden="true" />
      <p>Coming soon</p>
    </section>
  );
}
