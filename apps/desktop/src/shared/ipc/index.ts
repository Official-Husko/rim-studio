// Public entry of shared/ipc: the only module that talks to the backend.
export { call, callCommand, getTransport, setTransport, transportKind } from './client';
export { connect, connection, type ConnectionState } from './connection';
export { devCommands, devFsHome, devFsList, devInfo } from './dev';
export { clientErrorId, isApiError, normalizeError } from './error';
export {
  applyEvent as applyBridgeEvent,
  clearFinishedJobs,
  dismissJob,
  jobs,
  runningCount,
  type JobState,
} from './jobs';
export { IpcScope, useIpcScope } from './lifetime';
export { clearQueries, invalidate, query, useQuery, type QueryHandle } from './query';
export type {
  ApiError,
  BridgeEvent,
  DevCommandRow,
  DevFsEntry,
  DevFsHome,
  DevFsListing,
  DevInfo,
  Transport,
} from './types';
