import { Button } from 'rimstudio-ui';

/** One thing a Combat Extended patch is still waiting for. */
export interface MissingItem {
  /** Stable key, usually the field pointer of the answer. */
  key: string;
  /** What is missing, in the words of the question. */
  label: string;
  /** Why the generator needs it, when it says. */
  why?: string | undefined;
}

export interface MissingListProps {
  /** The heading, for example "2 answers are missing before the patch can be written". */
  title: string;
  items: readonly MissingItem[];
  /** The text of the shortcut button of each row. */
  goLabel: string;
  /** Called with the key of a row; absent hides the buttons. */
  onGoTo?: ((key: string) => void) | undefined;
  /** One line under the list, for example what happens when everything is answered. */
  footer?: string | undefined;
}

/**
 * The list of what is missing before a patch can be generated: one row per open answer with the
 * reason it is needed and a shortcut to the field. Renders nothing when nothing is missing.
 */
export function MissingList({ title, items, goLabel, onGoTo, footer }: MissingListProps) {
  if (items.length === 0) return null;
  return (
    <section aria-label={title} class="flex flex-col gap-2 border border-warning bg-surface p-3">
      <h3 class="m-0 text-body font-semibold text-fg">{title}</h3>
      <ul class="m-0 flex list-none flex-col gap-2 p-0">
        {items.map((item) => (
          <li key={item.key} class="flex items-start gap-2 text-body">
            <span
              aria-hidden="true"
              class="mt-1 inline-block size-3.5 shrink-0 border border-warning"
            />
            <span class="min-w-0 flex-1">
              <span class="break-words">{item.label}</span>
              {item.why ? <span class="block text-small text-muted">{item.why}</span> : null}
            </span>
            {onGoTo ? (
              <Button size="sm" variant="ghost" onClick={() => onGoTo(item.key)}>
                {goLabel}
              </Button>
            ) : null}
          </li>
        ))}
      </ul>
      {footer ? <p class="m-0 text-small text-muted">{footer}</p> : null}
    </section>
  );
}
