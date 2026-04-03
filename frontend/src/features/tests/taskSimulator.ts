export interface SimulatedTaskPlan {
  id: string;
  label: string;
  durationMs: number;
  tickMs: number;
}

const taskActionPool = [
  'Indexing biome defs',
  'Saving compatibility metadata',
  'Generating patch skeletons',
  'Scanning apparel textures',
  'Refreshing race references',
  'Resolving mod package IDs',
  'Parsing weapon def nodes',
  'Rebuilding live game cache',
  'Collecting DLC manifests',
  'Writing project config',
  'Checking patch entry collisions',
  'Resolving texture aliases',
  'Loading selected compatibility targets',
  'Validating About.xml fields',
  'Refreshing inherited stat bases',
  'Linking apparel tags',
  'Indexing scenario dependencies',
  'Enumerating faction defs',
  'Collecting pawn kind references',
  'Parsing damage profile curves',
  'Reconciling recipe users',
  'Scanning sound definition hooks',
  'Updating workbench bindings',
  'Rebuilding patch operation order',
  'Mapping body part groups',
  'Checking language data keys',
  'Refreshing icon atlases',
  'Evaluating extension nodes',
  'Compacting generated patch notes',
  'Staging XML export payloads',
];

function shuffle<T>(items: T[], rng: () => number) {
  const next = [...items];

  for (let index = next.length - 1; index > 0; index -= 1) {
    const swapIndex = Math.floor(rng() * (index + 1));
    const current = next[index];
    next[index] = next[swapIndex];
    next[swapIndex] = current;
  }

  return next;
}

function randomBetween(min: number, max: number, rng: () => number) {
  return Math.round(min + rng() * (max - min));
}

export function buildSimulatedTaskPlans(count: number, rng: () => number = Math.random): SimulatedTaskPlan[] {
  const safeCount = Math.max(1, Math.min(12, Math.floor(count || 1)));
  const labels = shuffle(taskActionPool, rng).slice(0, safeCount);
  const batchSeed = `${Date.now()}-${Math.floor(rng() * 100000)}`;

  return labels.map((label, index) => ({
    id: `demo-task-${batchSeed}-${index}`,
    label,
    durationMs: randomBetween(3500, 12500, rng),
    tickMs: randomBetween(180, 420, rng),
  }));
}
