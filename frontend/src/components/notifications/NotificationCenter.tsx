import { h } from 'preact';
import type { NotificationItem } from '../../types';
import { NotificationBubble } from './NotificationBubble';

interface Props {
  notifications: NotificationItem[];
  onDismiss: (id: string) => void;
}

export function NotificationCenter({ notifications, onDismiss }: Props) {
  if (notifications.length === 0) {
    return null;
  }

  return (
    <aside className="notification-center" aria-live="polite" aria-label="Notifications">
      {notifications.map((notification) => (
        <NotificationBubble key={notification.id} notification={notification} onDismiss={onDismiss} />
      ))}
    </aside>
  );
}
