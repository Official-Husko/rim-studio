import { cx } from '../cx';

export interface RulerMark {
  value: number;
  label: string;
  /** Reference marks are taller; the item's own mark is drawn in the accent colour. */
  kind?: 'reference' | 'item';
}

export interface RulerSliderProps {
  value: number;
  onValueChange: (value: number) => void;
  min: number;
  max: number;
  /** The natural step of the field; the slider snaps to it and arrow keys move by it. */
  step: number;
  /** Accessible name, for example "Damage". */
  label: string;
  unit?: string;
  /** Reference marks above the track (pool minimum, median, maximum, nearest items). */
  marks?: RulerMark[];
  p50?: [number, number];
  p80?: [number, number];
  /** The suggested value, drawn as a diamond marker. */
  suggested?: number;
  disabled?: boolean;
}

function pct(value: number, min: number, max: number): number {
  if (!(max > min)) return 0;
  return Math.min(100, Math.max(0, ((value - min) / (max - min)) * 100));
}

const MINOR_TICKS = 20;

function tickLabel(value: number): string {
  return String(Number(value.toFixed(2)));
}

/**
 * A slider whose track is a ruler: minor ticks every 5 percent, major ticks every 25 percent,
 * reference marks above, the P50 and P80 bands behind. The control is a native range input,
 * so pointer, keyboard and screen reader behaviour come from the browser.
 */
export function RulerSlider({
  value,
  onValueChange,
  min,
  max,
  step,
  label,
  unit,
  marks = [],
  p50,
  p80,
  suggested,
  disabled,
}: RulerSliderProps) {
  const band = (range: [number, number]) => {
    const left = pct(range[0], min, max);
    return { left: `${left}%`, width: `${Math.max(0, pct(range[1], min, max) - left)}%` };
  };
  const ticks = Array.from({ length: MINOR_TICKS + 1 }, (_, i) => i);
  return (
    <div class={cx('flex flex-col', disabled && 'opacity-50')}>
      <div class="relative h-9" aria-hidden="true">
        {marks.map((m) => (
          <span
            key={`${m.kind ?? 'reference'}-${m.value}-${m.label}`}
            data-mark={m.kind ?? 'reference'}
            class={cx(
              'absolute -translate-x-1/2 font-mono text-mono-small whitespace-nowrap',
              m.kind === 'item' ? 'top-4 text-accent' : 'top-0 text-faint',
            )}
            style={{ left: `${pct(m.value, min, max)}%` }}
          >
            {m.label}
          </span>
        ))}
        {marks.map((m) => (
          <span
            key={`tick-${m.kind ?? 'reference'}-${m.value}-${m.label}`}
            class={cx('absolute bottom-0 h-2 w-px', m.kind === 'item' ? 'bg-accent' : 'bg-faint')}
            style={{ left: `${pct(m.value, min, max)}%` }}
          />
        ))}
      </div>
      <div class="relative h-control">
        {p80 ? (
          <div data-band="p80" class="bp-hatch absolute inset-y-1 bg-band-p80" style={band(p80)} />
        ) : null}
        {p50 ? (
          <div data-band="p50" class="absolute inset-y-1 bg-band-p50" style={band(p50)} />
        ) : null}
        <div
          class="absolute inset-x-0 top-1/2 h-1 -translate-y-1/2 bg-line-strong"
          aria-hidden="true"
        />
        {suggested !== undefined ? (
          <span
            role="img"
            aria-label={`Suggested ${tickLabel(suggested)}${unit ? ` ${unit}` : ''}`}
            data-marker="suggested"
            class="absolute top-0 -ml-1 h-2 w-2 rotate-45 bg-info"
            style={{ left: `${pct(suggested, min, max)}%` }}
          />
        ) : null}
        <input
          type="range"
          class="rs-range absolute inset-0 w-full"
          aria-label={label}
          aria-valuetext={`${tickLabel(value)}${unit ? ` ${unit}` : ''}`}
          min={min}
          max={max}
          step={step}
          value={value}
          disabled={disabled}
          onInput={(e) => onValueChange(Number(e.currentTarget.value))}
        />
      </div>
      <div class="relative h-6" aria-hidden="true">
        {ticks.map((i) => {
          const major = i % 5 === 0;
          const at = min + ((max - min) * i) / MINOR_TICKS;
          return (
            <span key={i} class="absolute top-0 -translate-x-1/2" style={{ left: `${i * 5}%` }}>
              <span class={cx('mx-auto block w-px bg-line-strong', major ? 'h-2' : 'h-1')} />
              {major ? (
                <span class="font-mono text-mono-small text-faint">{tickLabel(at)}</span>
              ) : null}
            </span>
          );
        })}
      </div>
    </div>
  );
}
