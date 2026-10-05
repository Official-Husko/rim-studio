import type { ComponentChildren } from 'preact';

/** The small uppercase heading that names a block of a wizard step. */
export function SectionHeading({ children }: { children: ComponentChildren }) {
  return (
    <h3 class="font-display text-label font-semibold tracking-label text-muted uppercase">
      {children}
    </h3>
  );
}
