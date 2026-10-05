import type { ComponentChildren } from 'preact';

/** A titled block of the gallery; the id lets the toolbar link to it. */
export function Section({
  id,
  title,
  children,
}: {
  id: string;
  title: string;
  children: ComponentChildren;
}) {
  return (
    <section id={id} aria-labelledby={`${id}-h`} class="border-b border-line pb-6">
      <h2
        id={`${id}-h`}
        class="mb-3 font-display text-label font-semibold tracking-label text-muted uppercase"
      >
        {title}
      </h2>
      <div class="flex flex-col gap-4">{children}</div>
    </section>
  );
}

/** A labelled example inside a section. */
export function Demo({ label, children }: { label: string; children: ComponentChildren }) {
  return (
    <div class="flex flex-col gap-2">
      <h3 class="text-small text-faint">{label}</h3>
      <div class="flex flex-wrap items-center gap-3">{children}</div>
    </div>
  );
}
