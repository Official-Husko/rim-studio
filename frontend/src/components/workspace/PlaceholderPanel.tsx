import { h } from 'preact';

interface Props {
  label: string;
  copy: string;
}

export function PlaceholderPanel({ label, copy }: Props) {
  return (
    <section className="panel-card placeholder-panel">
      <p className="eyebrow">Workspace Placeholder</p>
      <h3>{label}</h3>
      <p>{copy}</p>
    </section>
  );
}
