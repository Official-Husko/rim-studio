import { describe, expect, it } from 'vitest';
import type { JsonValue, ResolvedDefDto } from 'rimstudio-ipc-types';
import { fixture } from '../testSupport';
import { treeToXml } from './xml';

describe('treeToXml', () => {
  it('writes attributes, nesting and escapes text', () => {
    const tree: JsonValue = {
      tag: 'ThingDef',
      attrs: [['Name', 'A"B']],
      children: [
        { tag: 'label', attrs: [], children: ['a < b & c'] },
        { tag: 'tools', attrs: [], children: [{ tag: 'li', attrs: [], children: [] }] },
      ],
    };
    expect(treeToXml(tree)).toBe(
      [
        '<ThingDef Name="A&quot;B">',
        '  <label>a &lt; b &amp; c</label>',
        '  <tools>',
        '    <li />',
        '  </tools>',
        '</ThingDef>',
      ].join('\n'),
    );
  });

  it('shows the real definition of the bolt-action rifle', () => {
    const def = fixture<ResolvedDefDto>('defs-resolved-bolt-action-rifle');
    const xml = treeToXml(def.tree);
    expect(
      xml.startsWith('<ThingDef ParentName="BaseHumanMakeableGun" Name="Gun_BoltActionRifle">'),
    ).toBe(true);
    expect(xml).toContain('<WorkToMake>12000</WorkToMake>');
    expect(xml).toContain('<Mass>3.5</Mass>');
  });

  it('returns nothing for a value that is no node', () => {
    expect(treeToXml('text')).toBe('');
  });
});
