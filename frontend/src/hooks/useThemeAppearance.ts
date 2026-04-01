import { useEffect } from 'preact/hooks';
import { api } from '../api';
import type { GlobalSettings, NotificationItem } from '../types';
import { getErrorMessage } from '../utils/ui';

const customThemeStyleID = 'rimstudio-custom-theme';
const customThemeNotificationKey = 'theme:custom-css';

interface NotificationTools {
  removeNotificationByKey: (key: string) => void;
  upsertNotification: (notification: Omit<NotificationItem, 'id'> & { key: string }) => void;
}

function ensureCustomThemeStyleElement() {
  let node = document.getElementById(customThemeStyleID) as HTMLStyleElement | null;
  if (!node) {
    node = document.createElement('style');
    node.id = customThemeStyleID;
    document.head.appendChild(node);
  }
  return node;
}

export function useThemeAppearance(settings: GlobalSettings, { removeNotificationByKey, upsertNotification }: NotificationTools) {
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', settings.themeId);
  }, [settings.themeId]);

  useEffect(() => {
    const styleNode = ensureCustomThemeStyleElement();
    const customCSSPath = settings.customCssPath.trim();
    let cancelled = false;

    if (!customCSSPath) {
      styleNode.textContent = '';
      removeNotificationByKey(customThemeNotificationKey);
      return undefined;
    }

    void api.readCustomCSSFile(customCSSPath)
      .then((css) => {
        if (cancelled) {
          return;
        }

        styleNode.textContent = css;
        removeNotificationByKey(customThemeNotificationKey);
      })
      .catch((error) => {
        if (cancelled) {
          return;
        }

        styleNode.textContent = '';
        upsertNotification({
          key: customThemeNotificationKey,
          type: 'warning',
          title: 'Custom theme CSS could not be loaded',
          message: getErrorMessage(error),
          icon: 'fa-file-circle-xmark',
          persistent: true,
        });
      });

    return () => {
      cancelled = true;
    };
  }, [removeNotificationByKey, settings.customCssPath, upsertNotification]);
}
