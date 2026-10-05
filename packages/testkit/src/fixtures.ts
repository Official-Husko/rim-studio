// Fixture loader: every JSON file in packages/testkit/fixtures is bundled eagerly, so the same
// loader works in vitest and in the browser (mock mode). A feature adds fixtures by adding
// files named after the command (designer_preview.json) or any other stable name.
const modules = import.meta.glob<unknown>('../fixtures/*.json', { eager: true, import: 'default' });

const byName = new Map<string, unknown>();
for (const [path, value] of Object.entries(modules)) {
  const file = path.split('/').pop() ?? path;
  byName.set(file.replace(/\.json$/, ''), value);
}

/** True when fixtures/<name>.json exists. */
export function hasFixture(name: string): boolean {
  return byName.has(name);
}

/** The parsed JSON of fixtures/<name>.json; throws when it does not exist. */
export function loadFixture<T = unknown>(name: string): T {
  if (!byName.has(name)) throw new Error(`No fixture named ${name} in packages/testkit/fixtures.`);
  return structuredClone(byName.get(name)) as T;
}

/** Every fixture name, sorted. */
export function fixtureNames(): string[] {
  return [...byName.keys()].sort();
}
