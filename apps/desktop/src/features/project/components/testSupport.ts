import { createMockTransport, loadFixture, type MockHandler } from 'rimstudio-testkit';
import type { ProjectLinkResultDto, ProjectLinkStatusDto } from 'rimstudio-ipc-types';
import { clearQueries, connection, setTransport } from '~/shared/ipc';
import { resetLinkStore } from './linkStore';

/** The project the link fixtures were recorded for. */
export const LINK_PROJECT_ID = 'p-f7cbde43';
export const LINK_PROJECT_PATH = '/home/pawbeans/Projects/RimWorld Mods/RS_Arms';

/** A recorded status with some fields replaced. */
export function statusFixture(
  name: string,
  patch: Partial<ProjectLinkStatusDto> = {},
): ProjectLinkStatusDto {
  return { ...loadFixture<ProjectLinkStatusDto>(name), ...patch };
}

/** A recorded result with some status fields replaced. */
export function resultFixture(
  name: string,
  patch: Partial<ProjectLinkStatusDto> = {},
): ProjectLinkResultDto {
  const result = loadFixture<ProjectLinkResultDto>(name);
  return { ...result, status: { ...result.status, ...patch } };
}

/** Reset the link store and install a mock transport; handlers win over the fixtures by command name. */
export function installLinkTransport(handlers: Record<string, MockHandler> = {}) {
  resetLinkStore();
  clearQueries();
  const transport = createMockTransport({ handlers });
  setTransport(transport);
  connection.value = 'mock';
  return transport;
}
