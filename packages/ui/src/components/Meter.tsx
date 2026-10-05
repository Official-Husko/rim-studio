import { cx } from '../cx';

/** How a value sits against the pool of reference items; computed by the backend. */
export type FitLevel = 'typical' | 'plausible' | 'unusual' | 'unknown';

export interface MeterProps {
  /** The item's value. */
  value: number;
  /** The scale of the bar. */
  min: number;
  max: number;
  /** The P50 band as [low, high] in the same unit. */
  p50?: [number, number];
  /** The P80 band as [low, high]. */
  p80?: [number, number];
  /** The prediction, drawn as a tick when present. */
  prediction?: number;
  level: FitLevel;
  /** Stat name, for example "Hit adjusted DPS". */
  label: string;
  unit?: string;
  /** The word for the level, already translated. Defaults to the English level name. */
  levelText?: string;
  /** The value as text, already formatted. */
  valueText?: string;
}

const LEVEL_TEXT: Record<FitLevel, string> = {
  typical: 'typical',
  plausible: 'plausible',
  unusual: 'unusual',
  unknown: 'unknown',
};
/** Green typical, blue plausible, amber unusual; never red. */
const LEVEL_COLOUR: Record<FitLevel, string> = {
  typical: 'text-success',
  plausible: 'text-info',
  unusual: 'text-warning',
  unknown: 'text-muted',
};

function pct(value: number, min: number, max: number): number {
  if (!(max > min)) return 0;
  return Math.min(100, Math.max(0, ((value - min) / (max - min)) * 100));
}

/** The shape beside the word: circle for typical, square for plausible, triangle for unusual. */
function LevelShape({ level }: { level: FitLevel }) {
  return (
    <svg viewBox="0 0 12 12" class="size-3 shrink-0" aria-hidden="true" fill="currentColor">
      {level === 'typical' ? <circle cx="6" cy="6" r="5" /> : null}
      {level === 'plausible' ? <rect x="1" y="1" width="10" height="10" /> : null}
      {level === 'unusual' ? <path d="M6 0.5 11.5 11H0.5z" /> : null}
      {level === 'unknown' ? <path d="M1 6h10" stroke="currentColor" stroke-width="2" /> : null}
    </svg>
  );
}

/** The fit meter: a ruler with P50 and P80 bands and a caret for the value. */
export function Meter({
  value,
  min,
  max,
  p50,
  p80,
  prediction,
  level,
  label,
  unit,
  levelText,
  valueText,
}: MeterProps) {
  const word = levelText ?? LEVEL_TEXT[level];
  const shownValue = valueText ?? `${value}${unit ? ` ${unit}` : ''}`;
  const band = (range: [number, number]) => {
    const left = pct(range[0], min, max);
    return { left: `${left}%`, width: `${Math.max(0, pct(range[1], min, max) - left)}%` };
  };
  return (
    <div class="flex flex-col gap-1">
      <div class="flex items-baseline justify-between gap-3">
        <span class="text-body text-fg">{label}</span>
        <span class={cx('inline-flex items-center gap-1 font-mono text-mono', LEVEL_COLOUR[level])}>
          <LevelShape level={level} />
          <span>{shownValue}</span>
          <span class="text-small">{word}</span>
        </span>
      </div>
      <div
        role="meter"
        aria-label={label}
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={value}
        aria-valuetext={`${shownValue}, ${word}`}
        class="relative h-4"
      >
        <div class="absolute inset-x-0 top-1.5 h-1 bg-raised" />
        {p80 ? (
          <div data-band="p80" class="bp-hatch absolute top-1 h-2 bg-band-p80" style={band(p80)} />
        ) : null}
        {p50 ? (
          <div data-band="p50" class="absolute top-1 h-2 bg-band-p50" style={band(p50)} />
        ) : null}
        {prediction !== undefined ? (
          <div
            data-marker="prediction"
            class="absolute top-0.5 h-3 w-px bg-muted"
            style={{ left: `${pct(prediction, min, max)}%` }}
          />
        ) : null}
        <div
          data-marker="value"
          class={cx(
            'absolute top-0 -ml-1.5 flex h-4 w-3 items-center justify-center bg-bg',
            LEVEL_COLOUR[level],
          )}
          style={{ left: `${pct(value, min, max)}%` }}
        >
          <LevelShape level={level} />
        </div>
      </div>
      <div class="flex justify-between font-mono text-mono-small text-faint">
        <span>{min}</span>
        <span>{max}</span>
      </div>
    </div>
  );
}
