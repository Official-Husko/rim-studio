import { createMockTransport, loadFixture, type MockHandler } from 'rimstudio-testkit';
import type {
  DesignSpecDto,
  DesignerCloneResponse,
  DraftDto,
  DraftEntryDto,
} from 'rimstudio-ipc-types';
import { setTransport } from '~/shared/ipc';
import type { Scheduler } from './scheduler';

/** Install a mock transport; commands without a handler answer from the recorded fixtures. */
export function installTransport(handlers: Record<string, MockHandler> = {}) {
  const transport = createMockTransport({
    handlers: {
      designer_draft_save: () => ({ id: 'd-test', savedAtMs: 1000 }),
      designer_draft_list: () => ({ drafts: [] }),
      ...handlers,
    },
  });
  setTransport(transport);
  return transport;
}

/** A recorded fixture, typed by the caller. */
export function fixture<T>(name: string): T {
  return loadFixture<T>(name);
}

/** The clone of the bolt-action rifle with its damage edited, as a stored entry. */
export function cloneEntry(): DraftEntryDto {
  const draft = fixture<DraftDto>('designer-draft-clone-edited');
  return {
    id: 'd-clone',
    defName: draft.spec.identity.defName,
    label: draft.spec.identity.label,
    kind: 'ranged',
    updatedAtMs: 1,
    draft,
  };
}

/** The same rifle cloned with its own projectile (the default of a clone), damage 22. */
export function ownCloneEntry(): DraftEntryDto {
  const draft = fixture<DraftDto>('designer-fields-draft-rifle-own-edited');
  return {
    id: 'd-own',
    defName: draft.spec.identity.defName,
    label: draft.spec.identity.label,
    kind: 'ranged',
    updatedAtMs: 1,
    draft,
  };
}

/** A recorded clone of a real weapon: designer-fields-clone-NAME. */
export function recordedSpec(name: string): DesignSpecDto {
  return fixture<DesignerCloneResponse>(`designer-fields-clone-${name}`).entry.draft.spec;
}

/** A scheduler that runs nothing until the test says so. */
export function manualScheduler() {
  let frames: Array<() => void> = [];
  let timers: Array<{ ms: number; run: () => void }> = [];
  const scheduler: Scheduler = {
    frame(run) {
      frames.push(run);
      return () => {
        frames = frames.filter((f) => f !== run);
      };
    },
    after(ms, run) {
      const entry = { ms, run };
      timers.push(entry);
      return () => {
        timers = timers.filter((t) => t !== entry);
      };
    },
  };
  return {
    scheduler,
    /** Run the queued animation frames. */
    runFrames(): void {
      const due = frames;
      frames = [];
      for (const run of due) run();
    },
    /** Run every queued timer, whatever its delay. */
    runTimers(): void {
      const due = timers;
      timers = [];
      for (const { run } of due) run();
    },
    pending: () => ({ frames: frames.length, timers: timers.map((t) => t.ms) }),
  };
}

/** Let the promises of the mock transport settle. */
export async function settle(): Promise<void> {
  for (let i = 0; i < 6; i += 1) await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

/** The draft of a clone with two textures and two shot clips imported (recorded from the real bridge). */
export function importsDraft(): DraftDto {
  return fixture<DraftDto>('designer-assets-draft-imports');
}

/** The facts the real bridge gave for the recorded import files, picked by the file name asked for. */
export function recordedAssetInfo(request: unknown): unknown {
  const path = (request as { path: string }).path;
  if (path.endsWith('Eagle_Carbine.png')) return fixture('designer-assets-info-png');
  if (path.endsWith('RGD_5_Projectile.png')) return fixture('designer-assets-info-projectile-png');
  if (path.endsWith('Glock_17_Shot.wav')) return fixture('designer-assets-info-wav');
  if (path.endsWith('AK_47_Shot.wav')) return fixture('designer-assets-info-wav2');
  return fixture('designer-assets-info-missing');
}

/** A mock transport that answers `designer_asset_info` and `defs_search` like the recorded bridge. */
export function installAssetTransport(handlers: Record<string, MockHandler> = {}) {
  return installTransport({
    designer_asset_info: recordedAssetInfo,
    defs_search: (request) =>
      (request as { query: string }).query === ''
        ? fixture('designer-assets-sounds-all')
        : fixture('designer-assets-sounds-search'),
    ...handlers,
  });
}
