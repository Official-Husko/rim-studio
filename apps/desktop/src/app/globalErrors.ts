import { normalizeError } from '~/shared/ipc';
import { t } from '~/shared/i18n';
import { pushToast } from './toasts';

let installed = false;

/** Surface unhandled errors and rejections as one toast each, with the error id. */
export function installGlobalErrorToasts(): () => void {
  if (installed) return () => {};
  installed = true;
  const onError = (event: ErrorEvent): void => {
    const error = normalizeError(event.error ?? event.message);
    pushToast({
      tone: 'error',
      message: t('error.unhandled', { message: error.message }),
      title: t('error.id', { id: error.errorId }),
    });
  };
  const onRejection = (event: PromiseRejectionEvent): void => {
    const error = normalizeError(event.reason);
    pushToast({
      tone: 'error',
      message: t('error.unhandled', { message: error.message }),
      title: t('error.id', { id: error.errorId }),
    });
  };
  window.addEventListener('error', onError);
  window.addEventListener('unhandledrejection', onRejection);
  return () => {
    window.removeEventListener('error', onError);
    window.removeEventListener('unhandledrejection', onRejection);
    installed = false;
  };
}
