import { useRef, useState } from 'preact/hooks';
import type { NotificationItem } from '../types';

export function useNotifications() {
  const [notifications, setNotifications] = useState<NotificationItem[]>([]);
  const notificationCounter = useRef(0);

  function notify(notification: Omit<NotificationItem, 'id'>) {
    notificationCounter.current += 1;
    const id = `notification-${notificationCounter.current}`;

    setNotifications((current) => [
      ...current,
      {
        ...notification,
        id,
        durationMs: notification.persistent ? notification.durationMs : (notification.durationMs ?? 10000),
      },
    ]);
  }

  function notifyError(title: string, message: string) {
    notify({
      type: 'error',
      title,
      message,
      icon: 'fa-circle-exclamation',
    });
  }

  function dismissNotification(id: string) {
    setNotifications((current) => current.filter((notification) => notification.id !== id));
  }

  function removeNotificationByKey(key: string) {
    setNotifications((current) => current.filter((notification) => notification.key !== key));
  }

  function upsertNotification(notification: Omit<NotificationItem, 'id'> & { key: string }) {
    setNotifications((current) => {
      const existing = current.find((item) => item.key === notification.key);
      if (existing) {
        return current.map((item) => (item.key === notification.key ? { ...item, ...notification, id: item.id } : item));
      }

      notificationCounter.current += 1;
      return [
        ...current,
        {
          ...notification,
          id: `notification-${notificationCounter.current}`,
        },
      ];
    });
  }

  return {
    notifications,
    notify,
    notifyError,
    dismissNotification,
    removeNotificationByKey,
    upsertNotification,
  };
}
