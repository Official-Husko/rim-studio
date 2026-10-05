import { useRef } from 'preact/hooks';
import { cx } from '../cx';
import { Icon, type IconName } from './Icon';

export interface SegmentOption {
  value: string;
  label: string;
  icon?: IconName;
  disabled?: boolean;
}

export interface SegmentedControlProps {
  options: SegmentOption[];
  value: string;
  onValueChange: (value: string) => void;
  /** Accessible name of the group. */
  label: string;
}

/** A radio group drawn as joined buttons. Arrow keys move and select; one tab stop. */
export function SegmentedControl({ options, value, onValueChange, label }: SegmentedControlProps) {
  const refs = useRef<Array<HTMLButtonElement | null>>([]);
  const enabled = options.map((o, i) => (o.disabled ? -1 : i)).filter((i) => i >= 0);

  const move = (from: number, delta: 1 | -1): void => {
    const pos = enabled.indexOf(from);
    const target = enabled[(pos + delta + enabled.length) % enabled.length];
    const option = target === undefined ? undefined : options[target];
    if (option && target !== undefined) {
      onValueChange(option.value);
      refs.current[target]?.focus();
    }
  };

  const activeIndex = options.findIndex((o) => o.value === value);
  const tabStop = activeIndex >= 0 && !options[activeIndex]?.disabled ? activeIndex : enabled[0];

  return (
    <div
      role="radiogroup"
      aria-label={label}
      class="inline-flex rounded-md border border-line-strong bg-raised"
    >
      {options.map((o, i) => {
        const selected = o.value === value;
        return (
          <button
            key={o.value}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role="radio"
            aria-checked={selected ? 'true' : 'false'}
            disabled={o.disabled}
            tabIndex={i === tabStop ? 0 : -1}
            onClick={() => onValueChange(o.value)}
            onKeyDown={(e) => {
              if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
                e.preventDefault();
                move(i, 1);
              } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
                e.preventDefault();
                move(i, -1);
              }
            }}
            class={cx(
              'inline-flex h-control-sm items-center gap-1 px-3 text-body whitespace-nowrap first:rounded-l-md last:rounded-r-md',
              'disabled:cursor-not-allowed disabled:opacity-50',
              selected ? 'bg-accent font-semibold text-on-accent' : 'text-fg hover:bg-hover',
            )}
          >
            {o.icon ? <Icon name={o.icon} /> : null}
            {o.label}
          </button>
        );
      })}
    </div>
  );
}
