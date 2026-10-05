import type { Transport } from '../types';

/** Load the fixture backed mock transport of the testkit (a separate chunk, only used without a bridge). */
export async function createMockTransport(): Promise<Transport> {
  const { createMockTransport: create } = await import('rimstudio-testkit/mock');
  return create();
}
