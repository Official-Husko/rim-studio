import { h } from 'preact';
import { useEffect, useRef, useState } from 'preact/hooks';
import type { NotificationItem } from '../../types';

interface Props {
  notification: NotificationItem;
  onDismiss: (id: string) => void;
}

export function NotificationBubble({ notification, onDismiss }: Props) {
  const [isLeaving, setIsLeaving] = useState(false);
  const dismissTimerRef = useRef<number | null>(null);
  const removeTimerRef = useRef<number | null>(null);
  const autoDismissDelay = notification.durationMs ?? 10000;

  useEffect(() => {
    if (notification.persistent) {
      return undefined;
    }

    dismissTimerRef.current = window.setTimeout(() => {
      beginDismiss();
    }, autoDismissDelay);

    return () => {
      if (dismissTimerRef.current !== null) {
        window.clearTimeout(dismissTimerRef.current);
      }
      if (removeTimerRef.current !== null) {
        window.clearTimeout(removeTimerRef.current);
      }
    };
  }, [autoDismissDelay, notification.id, notification.persistent]);

  function beginDismiss() {
    if (isLeaving) {
      return;
    }

    if (dismissTimerRef.current !== null) {
      window.clearTimeout(dismissTimerRef.current);
    }

    setIsLeaving(true);
    removeTimerRef.current = window.setTimeout(() => {
      onDismiss(notification.id);
    }, 240);
  }

  return (
    <article
      className={`notification-bubble notification-${notification.type} ${isLeaving ? 'is-leaving' : ''}`}
      style={{ '--notification-duration': `${autoDismissDelay}ms` } as h.JSX.CSSProperties}
    >
      <div className="notification-icon">
        <i className={`fa-solid ${notification.icon}`} aria-hidden="true" />
      </div>

      <div className="notification-body">
        <div className="notification-header">
          <strong>{notification.title}</strong>
          <button
            aria-label="Dismiss notification"
            className="notification-close"
            onClick={beginDismiss}
            type="button"
          >
            <i className="fa-solid fa-xmark" aria-hidden="true" />
          </button>
        </div>

        <p>{notification.message}</p>

        {notification.actions && notification.actions.length > 0 ? (
          <div className="notification-actions">
            {notification.actions.map((action) => (
              <button className="secondary-button notification-action" onClick={action.onClick} type="button">
                {action.label}
              </button>
            ))}
          </div>
        ) : null}

        {!notification.persistent ? (
          <div className="notification-progress" aria-hidden="true">
            <div className={`notification-progress-bar notification-progress-${notification.type}`} />
          </div>
        ) : null}
      </div>
    </article>
  );
}
