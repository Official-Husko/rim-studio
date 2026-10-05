import type {
  CeAttachmentLinkDto,
  CeOptionDto,
  CePatchSpecDto,
  CeStatEntryDto,
  CeToolPlanDto,
  RawNodeDto,
  SourcedDto,
} from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';

// Presentation helpers of the Combat Extended block editors. They move values between a form and the
// block; whether a block is valid, what a bow needs and what the numbers should be come from the backend.

/** A change to the block: a member set to a value, or removed when the value is undefined. */
export type CeBlockPatch = { [K in keyof CePatchSpecDto]?: CePatchSpecDto[K] | undefined };

/** The members a patch may carry; the two plain flags of the block are never touched by the editors. */
export type CeBlock = CePatchSpecDto;

function isEmptyList(value: unknown): boolean {
  return Array.isArray(value) && value.length === 0;
}

/** A copy of the block with the patch applied. An undefined member, an empty list and false are removed. */
export function applyPatch(block: CeBlock, patch: CeBlockPatch): CeBlock {
  const next: Record<string, unknown> = { ...block };
  for (const [key, value] of Object.entries(patch)) {
    const keepsFalse = key === 'oneHanded' || key === 'beltFed';
    if (value === undefined || isEmptyList(value) || (value === false && !keepsFalse)) {
      delete next[key];
    } else {
      next[key] = value;
    }
  }
  return next as CeBlock;
}

/** A typed number for a block member. */
export function typed(value: number): SourcedDto<number> {
  return { value, source: 'typed' };
}

/** A typed whole number for a block member. */
export function typedInt(value: number): SourcedDto<number> {
  return { value: Math.max(0, Math.round(value)), source: 'typed' };
}

/** One entry of a list replaced, or removed when `next` is undefined. */
export function replaceAt<T>(list: readonly T[], index: number, next: T | undefined): T[] {
  const copy = [...list];
  if (next === undefined) copy.splice(index, 1);
  else copy[index] = next;
  return copy;
}

/** The list with one entry moved by `delta` places; unchanged when it would leave the list. */
export function moveBy<T>(list: readonly T[], index: number, delta: number): T[] {
  const to = index + delta;
  const item = list[index];
  if (item === undefined || to < 0 || to >= list.length) return [...list];
  const copy = [...list];
  copy.splice(index, 1);
  copy.splice(to, 0, item);
  return copy;
}

/** A new attachment link with a name only. */
export function newLink(): CeAttachmentLinkDto {
  return { attachment: '' };
}

/** A new tool plan entry. */
export function newTool(label = ''): CeToolPlanDto {
  return { label };
}

/** A new stat entry. */
export function newStat(): CeStatEntryDto {
  return { stat: '', value: 0 };
}

/** The element a pair of fields describes: a tag with a text value. */
export function leafNode(tag: string, text: string): RawNodeDto {
  return { tag, attrs: [], children: text === '' ? [] : [text] };
}

/** Why a text is not an element tree, or undefined when it is one. */
export function treeProblem(text: string): 'json' | 'shape' | undefined {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return 'json';
  }
  if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) return 'shape';
  const tag = (parsed as { tag?: unknown }).tag;
  return typeof tag === 'string' && tag !== '' ? undefined : 'shape';
}

/** The tree of a text that passed [`treeProblem`]; the backend validates it again. */
export function parseTree(text: string): RawNodeDto {
  const parsed = JSON.parse(text) as Partial<RawNodeDto>;
  return { tag: parsed.tag ?? '', attrs: parsed.attrs ?? [], children: parsed.children ?? [] };
}

/** One line that says what an element is: its tag and, for a text only element, the text. */
export function nodeSummary(node: RawNodeDto): string {
  const only = node.children.length === 1 ? node.children[0] : undefined;
  return typeof only === 'string' ? `${node.tag}: ${only}` : node.tag;
}

/** The value of a suggested option as the patch that writes it into the block. */
export function optionPatch(option: CeOptionDto, block: CeBlock): CeBlockPatch {
  switch (option.value.kind) {
    case 'tags': {
      const have = block.extraTags ?? [];
      return { extraTags: [...have, ...option.value.value.filter((tag) => !have.includes(tag))] };
    }
    case 'flag':
      return { reloadOneAtATime: option.value.value };
    case 'text':
      return { recoilPattern: option.value.value };
    case 'tool-plan':
      return { toolPlan: option.value.value };
  }
}

/** True when the block already holds what the option would write. */
export function optionTaken(option: CeOptionDto, block: CeBlock): boolean {
  switch (option.value.kind) {
    case 'tags': {
      const have = block.extraTags ?? [];
      return option.value.value.every((tag) => have.includes(tag));
    }
    case 'flag':
      return block.reloadOneAtATime !== undefined;
    case 'text':
      return block.recoilPattern !== undefined;
    case 'tool-plan':
      return (block.toolPlan ?? []).length > 0;
  }
}

/** A short text of what an option would write. */
export function optionValueText(option: CeOptionDto): string {
  switch (option.value.kind) {
    case 'tags':
      return option.value.value.join(', ');
    case 'flag':
      return option.value.value ? t('ceblock.yes') : t('ceblock.no');
    case 'text':
      return option.value.value;
    case 'tool-plan':
      return option.value.value.map((tool) => tool.label || tool.from || '').join(', ');
  }
}
