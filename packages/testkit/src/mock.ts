// A smaller entry for the app: the mock transport and the fixtures, without the render helpers
// (and so without @testing-library). The desktop app loads it lazily when no bridge is running.
export { createMockTransport, mockError } from './mockTransport';
export type {
  MockApiError,
  MockBridgeEvent,
  MockHandler,
  MockTransport,
  MockTransportOptions,
} from './mockTransport';
export { fixtureNames, hasFixture, loadFixture } from './fixtures';
