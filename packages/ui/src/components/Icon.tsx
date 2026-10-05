import type { JSX } from 'preact';

/** The in house icon set: 16 px grid, 1.5 px stroke, square caps, mitred joins. */
const PATHS = {
  folder: 'M1.75 3.75h4l1.5 1.5h7v7.5h-12.5z',
  file: 'M3.75 1.75h5.5l3 3v9.5h-8.5z M9.25 1.75v3h3',
  xml: 'M5.5 4.5 2 8l3.5 3.5 M10.5 4.5 14 8l-3.5 3.5 M9 3.5l-2 9',
  image: 'M1.75 2.75h12.5v10.5h-12.5z M1.75 11l3.5-3.5 3 3 2-2 4 4 M10 5h1.5v1.5H10z',
  sound: 'M2 6h2.5l3.5-3v10l-3.5-3H2z M10.5 5.5c1 1.5 1 3.5 0 5 M12.5 3.5c2 2.5 2 6.5 0 9',
  patch: 'M2 2.75h8.5v8.5H2z M5.5 5.5h8.5v8.5H5.5z',
  warning: 'M8 1.75 14.75 13.75h-13.5z M8 6v3.5 M8 11.25v1',
  check: 'M2.5 8.5l3.5 3.5 7.5-8',
  plus: 'M8 2.5v11 M2.5 8h11',
  minus: 'M2.5 8h11',
  trash: 'M2.5 4h11 M6 4V2.5h4V4 M4 4l.75 9.5h6.5L12 4 M6.75 6.5v5 M9.25 6.5v5',
  copy: 'M5.5 5.5h8v8h-8z M10.5 5.5v-3h-8v8h3',
  play: 'M4.5 2.5v11l9-5.5z',
  chevron: 'M5.5 3 10.5 8 5.5 13',
  search: 'M6.75 11.5a4.75 4.75 0 1 0 0-9.5 4.75 4.75 0 0 0 0 9.5z M10.5 10.5l4 4',
  link: 'M6.5 9.5l3-3 M7.5 4.5l1-1a2.8 2.8 0 0 1 4 4l-1 1 M8.5 11.5l-1 1a2.8 2.8 0 0 1-4-4l1-1',
  lock: 'M3.5 7h9v7h-9z M5.5 7V4.75a2.5 2.5 0 0 1 5 0V7',
  refresh: 'M13.5 8a5.5 5.5 0 1 1-1.6-3.9 M13.5 2.5v3h-3',
  settings:
    'M8 10a2 2 0 1 0 0-4 2 2 0 0 0 0 4z M8 1.5v2 M8 12.5v2 M1.5 8h2 M12.5 8h2 M3.4 3.4l1.4 1.4 M11.2 11.2l1.4 1.4 M3.4 12.6l1.4-1.4 M11.2 4.8l1.4-1.4',
  close: 'M3.5 3.5l9 9 M12.5 3.5l-9 9',
  info: 'M2 2h12v12H2z M8 7v4.5 M8 4.5v1',
  error: 'M8 1.75a6.25 6.25 0 1 0 0 12.5 6.25 6.25 0 0 0 0-12.5z M8 4.75v4 M8 10.75v1',
  'arrow-up': 'M8 13V3 M3.5 7.5 8 3l4.5 4.5',
  'arrow-down': 'M8 3v10 M3.5 8.5 8 13l4.5-4.5',
  crosshair: 'M8 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6z M8 1.5v3 M8 11.5v3 M1.5 8h3 M11.5 8h3',
  grid: 'M2 2h5v5H2z M9 2h5v5H9z M2 9h5v5H2z M9 9h5v5H9z',
  tasks: 'M3 4h10 M3 8h10 M3 12h6',
  home: 'M2 7.5 8 2.5l6 5 M3.5 7v6.5h9V7',
} as const;

export type IconName = keyof typeof PATHS;

/** Every icon name, for the gallery and tests. */
export const ICON_NAMES = Object.keys(PATHS) as IconName[];

export interface IconProps extends Omit<JSX.SVGAttributes<SVGSVGElement>, 'size'> {
  /** Which glyph to draw. */
  name: IconName;
  /** Pixel size of the square box (16 in rows, 20 in toolbars). */
  size?: 16 | 20 | 24;
  /** Quarter turns clockwise; used to point the chevron. */
  turn?: 0 | 1 | 2 | 3;
  /** An accessible name. Without one the icon is decorative and hidden from assistive tech. */
  label?: string;
}

const TURNS = { 0: 'rotate-0', 1: 'rotate-90', 2: 'rotate-180', 3: '-rotate-90' } as const;
const SIZES = { 16: 'size-4', 20: 'size-5', 24: 'size-6' } as const;

/** An inline SVG icon drawn in the current text colour. */
export function Icon({ name, size = 16, turn = 0, label, class: className, ...rest }: IconProps) {
  return (
    <svg
      {...rest}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      stroke-width="1.5"
      stroke-linecap="square"
      stroke-linejoin="miter"
      role={label ? 'img' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : 'true'}
      focusable="false"
      data-icon={name}
      class={`${SIZES[size]} ${TURNS[turn]} shrink-0 ${typeof className === 'string' ? className : ''}`}
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
