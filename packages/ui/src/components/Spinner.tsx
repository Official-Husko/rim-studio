import { cx } from '../cx';

export interface SpinnerProps {
  /** Pixel size class: sm is 16 px, md is 24 px. */
  size?: 'sm' | 'md';
  /** Accessible name, announced politely. */
  label?: string;
}

/** An indeterminate spinner. Motion is removed under reduced motion by the base styles. */
export function Spinner({ size = 'sm', label = 'Loading' }: SpinnerProps) {
  return (
    <svg
      role="status"
      aria-label={label}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      class={cx('rs-spin shrink-0', size === 'sm' ? 'size-4' : 'size-6')}
    >
      <circle cx="8" cy="8" r="6" class="opacity-25" />
      <path d="M8 2a6 6 0 0 1 6 6" />
    </svg>
  );
}
