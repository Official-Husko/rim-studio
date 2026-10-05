import { Badge, Icon } from 'rimstudio-ui';
import { connection, type ConnectionState } from '~/shared/ipc';
import { t, type MessageKey } from '~/shared/i18n';

const TEXT: Record<ConnectionState, MessageKey> = {
  connecting: 'connection.connecting',
  bridge: 'connection.bridge',
  mock: 'connection.mock',
  tauri: 'connection.tauri',
  lost: 'connection.lost',
};

const TONE = {
  connecting: 'neutral',
  bridge: 'success',
  mock: 'info',
  tauri: 'success',
  lost: 'warning',
} as const;

/** The connection status chip of the top bar: bridge ok, mock data, and so on. */
export function ConnectionChip() {
  const state = connection.value;
  return (
    <span role="status" aria-label={t('connection.label')} data-state={state}>
      <Badge tone={TONE[state]}>
        <span class="inline-flex items-center gap-1">
          <Icon name={state === 'mock' ? 'file' : 'link'} />
          {t(TEXT[state])}
        </span>
      </Badge>
    </span>
  );
}
