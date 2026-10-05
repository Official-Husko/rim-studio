import type { ComponentChildren } from 'preact';
import { useEffect, useId, useRef } from 'preact/hooks';
import { cx } from '../cx';
import { IconButton } from './IconButton';

export interface DialogProps {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ComponentChildren;
  /** Buttons row, right aligned. */
  footer?: ComponentChildren;
  /** Accessible name of the close button. */
  closeLabel?: string;
  /** Wider dialog for browsers and previews. */
  size?: 'md' | 'lg';
  /** Closing by a click on the backdrop; off by default so a destructive confirm is deliberate. */
  closeOnBackdrop?: boolean;
}

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * A modal dialog on the native element: focus is trapped, Escape closes, and focus returns to
 * the opener. Reserve it for confirmations and short forms (see the modal policy of the brief).
 */
export function Dialog({
  open,
  title,
  onClose,
  children,
  footer,
  closeLabel = 'Close',
  size = 'md',
  closeOnBackdrop,
}: DialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();

  useEffect(() => {
    const dialog = ref.current;
    if (!open || !dialog) return undefined;
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (typeof dialog.showModal === 'function') {
      if (!dialog.open) dialog.showModal();
    } else {
      dialog.setAttribute('open', '');
    }
    const first =
      dialog.querySelector<HTMLElement>('[autofocus]') ??
      dialog.querySelector<HTMLElement>(FOCUSABLE);
    first?.focus();
    return () => {
      if (typeof dialog.close === 'function' && dialog.open) dialog.close();
      else dialog.removeAttribute('open');
      opener?.focus();
    };
  }, [open]);

  if (!open) return null;

  const onKeyDown = (event: KeyboardEvent): void => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== 'Tab') return;
    const dialog = ref.current;
    if (!dialog) return;
    const items = [...dialog.querySelectorAll<HTMLElement>(FOCUSABLE)];
    const firstItem = items[0];
    const lastItem = items[items.length - 1];
    if (!firstItem || !lastItem) return;
    const active = document.activeElement;
    if (event.shiftKey && (active === firstItem || !dialog.contains(active))) {
      event.preventDefault();
      lastItem.focus();
    } else if (!event.shiftKey && (active === lastItem || !dialog.contains(active))) {
      event.preventDefault();
      firstItem.focus();
    }
  };

  return (
    // The native dialog element handles keys for the focus trap and Escape; it is the modal itself.
    // oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions
    <dialog
      ref={ref}
      aria-labelledby={titleId}
      aria-modal="true"
      onKeyDown={onKeyDown}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      onClick={(e) => {
        if (closeOnBackdrop && e.target === ref.current) onClose();
      }}
      class="m-auto border-0 bg-transparent p-0 text-fg backdrop:bg-scrim"
    >
      <div
        class={cx(
          'bp-ticks flex max-h-[80vh] flex-col border border-line-strong bg-raised',
          size === 'md' ? 'w-dialog' : 'w-[min(90vw,calc(var(--rs-dialog-w)*1.6))]',
        )}
      >
        <header class="flex items-center justify-between gap-3 border-b border-line px-4 py-3">
          <h2 id={titleId} class="text-title font-semibold">
            {title}
          </h2>
          <IconButton icon="close" label={closeLabel} onClick={onClose} noTooltip />
        </header>
        <div class="min-h-0 flex-1 overflow-auto px-4 py-3">{children}</div>
        {footer ? (
          <footer class="flex justify-end gap-2 border-t border-line px-4 py-3">{footer}</footer>
        ) : null}
      </div>
    </dialog>
  );
}
