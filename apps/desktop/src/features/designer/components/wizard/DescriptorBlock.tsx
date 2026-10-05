import type { ComponentChildren } from 'preact';
import { SectionHeading } from './SectionHeading';

export interface DescriptorBlockProps {
  title: string;
  /** One plain line under the control: what the choice does. */
  help?: string | undefined;
  children: ComponentChildren;
}

/** A titled block of the Describe step: the name of the descriptor, its control, its effect. */
export function DescriptorBlock({ title, help, children }: DescriptorBlockProps) {
  return (
    <section class="flex min-w-0 flex-col gap-2" aria-label={title}>
      <SectionHeading>{title}</SectionHeading>
      {children}
      {help ? <p class="text-small text-muted">{help}</p> : null}
    </section>
  );
}
