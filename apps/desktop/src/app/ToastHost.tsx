import { Toast, ToastStack } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { dismissToast, toasts } from './toasts';

/** Renders the toast queue in the fixed stack at the bottom right. */
export function ToastHost() {
  return (
    <ToastStack label={t('toast.region')}>
      {toasts.value.map((item) => (
        <Toast
          key={item.id}
          tone={item.tone}
          title={item.title}
          message={item.message}
          actionLabel={item.actionLabel}
          onAction={item.onAction}
          dismissLabel={t('toast.dismiss')}
          onDismiss={() => dismissToast(item.id)}
        />
      ))}
    </ToastStack>
  );
}
