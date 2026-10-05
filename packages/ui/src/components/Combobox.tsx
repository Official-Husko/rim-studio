import { useId, useMemo, useState } from 'preact/hooks';
import { cx } from '../cx';
import { useFieldContext } from './fieldContext';

export interface ComboboxOption {
  value: string;
  label: string;
  /** Secondary text, shown muted after the label (a defName, a source). */
  hint?: string;
  disabled?: boolean;
}

export interface ComboboxProps {
  options: ComboboxOption[];
  value: string | undefined;
  onValueChange: (value: string) => void;
  'aria-label'?: string;
  id?: string;
  placeholder?: string;
  /** Shown when the filter matches nothing. */
  emptyText?: string;
  invalid?: boolean;
  disabled?: boolean;
}

/** A filterable single choice list. Typing filters by label and hint; arrows, Enter and Escape work. */
export function Combobox({
  options,
  value,
  onValueChange,
  placeholder,
  emptyText = 'No matches',
  disabled,
  ...rest
}: ComboboxProps) {
  const field = useFieldContext();
  const base = useId();
  const listId = `${base}-list`;
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState<string | null>(null);
  const [active, setActive] = useState(0);

  const selected = options.find((o) => o.value === value);
  const text = query ?? selected?.label ?? '';
  const visible = useMemo(() => {
    const q = (query ?? '').trim().toLowerCase();
    if (!q) return options;
    return options.filter(
      (o) => o.label.toLowerCase().includes(q) || (o.hint ?? '').toLowerCase().includes(q),
    );
  }, [options, query]);

  const close = (): void => {
    setOpen(false);
    setQuery(null);
  };
  const choose = (o: ComboboxOption | undefined): void => {
    if (!o || o.disabled) return;
    onValueChange(o.value);
    close();
  };
  const move = (delta: number): void => {
    if (visible.length === 0) return;
    setOpen(true);
    setActive((a) => (a + delta + visible.length) % visible.length);
  };
  const invalid = rest.invalid ?? field.invalid;
  const activeId = open && visible[active] ? `${base}-opt-${active}` : undefined;

  return (
    <div class="relative">
      <input
        id={rest.id ?? field.id}
        type="text"
        role="combobox"
        aria-label={rest['aria-label']}
        aria-describedby={field.describedBy}
        aria-invalid={invalid ? 'true' : undefined}
        aria-expanded={open ? 'true' : 'false'}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={activeId}
        autoComplete="off"
        disabled={disabled}
        placeholder={placeholder}
        value={text}
        onFocus={() => setOpen(true)}
        onBlur={close}
        onInput={(e) => {
          setQuery(e.currentTarget.value);
          setActive(0);
          setOpen(true);
        }}
        onKeyDown={(e) => {
          if (e.key === 'ArrowDown') {
            e.preventDefault();
            move(open ? 1 : 0);
          } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            move(-1);
          } else if (e.key === 'Enter' && open) {
            e.preventDefault();
            choose(visible[active]);
          } else if (e.key === 'Escape' && open) {
            e.preventDefault();
            close();
          }
        }}
        class={cx(
          'h-control w-full rounded-sm border bg-raised px-2 text-body text-fg placeholder:text-faint disabled:opacity-50',
          invalid ? 'border-danger' : 'border-line-strong hover:border-accent',
        )}
      />
      <ul
        id={listId}
        role="listbox"
        aria-label={rest['aria-label']}
        hidden={!open}
        class="absolute top-full right-0 left-0 z-(--rs-z-menu) mt-1 max-h-60 overflow-auto border border-line-strong bg-raised shadow-raised"
      >
        {visible.length === 0 ? (
          <li role="presentation" class="px-2 py-2 text-muted">
            {emptyText}
          </li>
        ) : (
          visible.map((o, i) => (
            <li
              key={o.value}
              id={`${base}-opt-${i}`}
              role="option"
              aria-selected={o.value === value ? 'true' : 'false'}
              aria-disabled={o.disabled ? 'true' : undefined}
              onMouseDown={(e) => {
                e.preventDefault();
                choose(o);
              }}
              onMouseMove={() => setActive(i)}
              class={cx(
                'flex min-h-control items-center justify-between gap-3 px-2 text-body',
                i === active && 'bg-hover',
                o.value === value && 'font-semibold text-accent',
                o.disabled && 'opacity-50',
              )}
            >
              <span class="truncate">{o.label}</span>
              {o.hint ? <span class="font-mono text-mono-small text-faint">{o.hint}</span> : null}
            </li>
          ))
        )}
      </ul>
    </div>
  );
}
