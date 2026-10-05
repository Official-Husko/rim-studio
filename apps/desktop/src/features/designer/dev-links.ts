import type { ItemKindDto, QuizAnswerDto } from 'rimstudio-ipc-types';
import { typed } from './model/draft';
import { selectProject } from './project-source';
import type { Designer } from './stores';

/**
 * Development deep links for the weapons page, read from the hash (for example
 * #/weapons?project=/tmp/Mod&clone=Gun_BoltActionRifle:My_Rifle&set=/ranged/damage=22). They let
 * a headless browser reach a state without clicks. They do nothing in a production build.
 */
export interface DevLinks {
  project?: string;
  /** Clone SOURCE into a draft named NAME: clone=SOURCE:NAME. */
  clone?: { source: string; name: string };
  /** Start a new draft: new=ranged:NAME or new=melee:NAME. */
  create?: { kind: ItemKindDto; name: string };
  /** Open an existing draft by def name. */
  open?: string;
  /** Tools to add before the numbers: tool=blade:Cut+Stab,handle:Blunt. */
  tools: Array<{ label: string; capacities: string[] }>;
  /** Typed numbers: set=/ranged/damage=22,/mass=3. */
  set: Array<[string, number]>;
  /** Open the quiz stepper and answer it: quiz=1&qa=tier:1,role:blade,weaker. */
  quiz: boolean;
  answers: QuizAnswerDto[];
  /** Show the real definition of a reference weapon. */
  def?: string;
  /** Click the structure suggestion once it is offered. */
  structure: boolean;
  /** Clone with the source projectile shared instead of an own one: shared=1. */
  shared: boolean;
  /** Open every collapsed panel of the editor once the draft is shown: expand=1. */
  expand: boolean;
}

/** Parse one answer such as tier:1, role:blade, typed:20 or weaker. */
export function parseAnswer(text: string): QuizAnswerDto | undefined {
  const [kind, value = ''] = text.split(':');
  switch (kind) {
    case 'tier':
      return { kind: 'tier', tier: Number(value) };
    case 'role':
      return { kind: 'role', role: value };
    case 'group':
      return { kind: 'group', group: value };
    case 'bin':
      return { kind: 'bin', index: Number(value) };
    case 'typed':
      return { kind: 'typed', value: Number(value) };
    case 'bucket':
      return { kind: 'bucket', bucket: value as 'lower' | 'similar' | 'higher' };
    case 'weaker':
    case 'same':
    case 'stronger':
    case 'closer-to-lower':
    case 'closer-to-upper':
    case 'not-sure':
    case 'skip':
    case 'use-what-i-have':
      return { kind };
    default:
      return undefined;
  }
}

/** Read the deep links out of a location hash. */
export function readDevLinks(hash: string): DevLinks {
  const at = hash.indexOf('?');
  const params = new URLSearchParams(at < 0 ? '' : hash.slice(at + 1));
  const links: DevLinks = {
    tools: [],
    set: [],
    quiz: false,
    answers: [],
    structure: params.get('structure') === '1',
    shared: params.get('shared') === '1',
    expand: params.get('expand') === '1',
  };
  const project = params.get('project');
  if (project) links.project = project;
  const clone = params.get('clone');
  if (clone?.includes(':')) {
    const [source = '', name = ''] = clone.split(':');
    links.clone = { source, name };
  }
  const create = params.get('new');
  if (create?.includes(':')) {
    const [kind = '', name = ''] = create.split(':');
    links.create = { kind: kind === 'melee' ? 'melee' : 'ranged', name };
  }
  const open = params.get('open');
  if (open) links.open = open;
  for (const pair of (params.get('set') ?? '').split(',').filter(Boolean)) {
    const [pointer = '', value = ''] = pair.split('=');
    if (pointer && value !== '' && Number.isFinite(Number(value)))
      links.set.push([pointer, Number(value)]);
  }
  for (const text of (params.get('tool') ?? '').split(',').filter(Boolean)) {
    const [label = '', caps = ''] = text.split(':');
    links.tools.push({ label, capacities: caps.split(/[+ ]/).filter(Boolean) });
  }
  links.quiz = params.get('quiz') === '1';
  for (const text of (params.get('qa') ?? '').split(',').filter(Boolean)) {
    const answer = parseAnswer(text);
    if (answer) links.answers.push(answer);
  }
  const def = params.get('def');
  if (def) links.def = def;
  return links;
}

/** Carry out the deep links in order; returns the def name to show, if any. */
export async function applyDevLinks(
  stores: Designer,
  links: DevLinks,
): Promise<string | undefined> {
  if (links.project) {
    await selectProject(links.project);
    await stores.drafts.load();
  }
  if (links.clone) {
    const entry = await stores.drafts.clone(
      links.clone.source,
      links.clone.name,
      undefined,
      links.shared ? false : undefined,
    );
    if (entry) await stores.select(entry);
  } else if (links.create) {
    const entry = await stores.drafts.create(
      links.create.kind,
      links.create.name,
      links.create.name,
    );
    if (entry) await stores.select(entry);
  } else if (links.open) {
    const entry = stores.drafts.entries.peek().find((e) => e.defName === links.open);
    if (entry) await stores.select(entry);
  }
  for (const tool of links.tools) {
    const count = stores.editor.draft.peek()?.spec.tools?.length ?? 0;
    stores.editor.setField(`/tools/${count}`, tool);
  }
  for (const [pointer, value] of links.set) stores.editor.setField(pointer, typed(value));
  if (links.structure) {
    for (let i = 0; i < 30 && !stores.editor.structure.peek()?.filled?.length; i += 1) {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    stores.editor.applyStructure();
  }
  if (links.quiz) {
    await stores.quiz.start();
    for (const answer of links.answers) await stores.quiz.answer(answer);
  }
  if (links.expand) {
    await new Promise((resolve) => setTimeout(resolve, 600));
    for (const button of document.querySelectorAll<HTMLElement>('button[aria-expanded="false"]'))
      button.click();
  }
  return links.def;
}
