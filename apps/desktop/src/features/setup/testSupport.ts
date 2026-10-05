import { createMockTransport, loadFixture, type MockHandler } from 'rimstudio-testkit';
import { clearQueries, connection, setTransport } from '~/shared/ipc';
import { resetSetupStores } from './stores';

/** Handlers answering every setup command from the fixtures recorded on a real install. */
export function setupHandlers(): Record<string, MockHandler> {
  return {
    detect_get_report: () => ({ report: loadFixture('detect-run') }),
    detect_run: () => loadFixture('detect-run'),
    detect_set_override: () => ({ report: loadFixture('detect-run') }),
    sources_list: () => loadFixture('sources-list-default'),
    sources_probe_folder: () => loadFixture('probe-mods-folder'),
    sources_add_folder: () => loadFixture('source-added'),
    sources_update: () => loadFixture('source-added'),
    sources_remove: () => ({ removed: true }),
    library_scan: () => loadFixture('library-scan'),
    settings_get: () => loadFixture('settings-get'),
  };
}

/** Install a mock transport for a test; extra handlers win over the fixture ones. */
export function installTransport(extra: Record<string, MockHandler> = {}) {
  clearQueries();
  resetSetupStores();
  const transport = createMockTransport({ handlers: { ...setupHandlers(), ...extra } });
  setTransport(transport);
  connection.value = 'mock';
  return transport;
}
