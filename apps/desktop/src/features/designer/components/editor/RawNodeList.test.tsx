import { fireEvent, screen, within } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { PreviewDto } from 'rimstudio-ipc-types';
import { fixture, recordedSpec } from '../../testSupport';
import { makeEnv, WithEnv } from './fieldEnvTestkit';
import { RawNodeList } from './RawNodeList';

describe('RawNodeList', () => {
  const spec = recordedSpec('longsword');
  const fields = spec.extraFields ?? [];

  it('shows each carried field of the real longsword as XML', () => {
    renderWithProviders(
      <WithEnv env={makeEnv({ spec })}>
        <RawNodeList pointer="/extraFields" label="Other definition fields" />
      </WithEnv>,
    );
    const region = screen.getByRole('region', { name: 'XML of relicChance' });
    expect(region.textContent).toContain('relicChance');
    expect(region.textContent).toContain('2');
    expect(screen.getByRole('region', { name: 'XML of thingSetMakerTags' }).textContent).toContain(
      'RewardStandardQualitySuper',
    );
  });

  it('edits a value element by tag and text and writes the node back at its place', () => {
    const env = makeEnv({ spec });
    const index = fields.findIndex((n) => n.tag === 'relicChance');
    renderWithProviders(
      <WithEnv env={env}>
        <RawNodeList pointer="/extraFields" label="Other definition fields" />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Edit relicChance' }));
    const dialog = screen.getByRole('dialog');
    fireEvent.input(within(dialog).getByLabelText('Text'), { target: { value: '5' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Save field' }));
    expect(env.setField).toHaveBeenCalledWith(`/extraFields/${index}`, {
      tag: 'relicChance',
      attrs: [],
      children: ['5'],
    });
  });

  it('edits a field with child elements as a tree', () => {
    const env = makeEnv({ spec });
    const index = fields.findIndex((n) => n.tag === 'thingSetMakerTags');
    renderWithProviders(
      <WithEnv env={env}>
        <RawNodeList pointer="/extraFields" label="Other definition fields" />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Edit thingSetMakerTags' }));
    const dialog = screen.getByRole('dialog');
    const tree = within(dialog).getByLabelText('Node tree (JSON)') as HTMLTextAreaElement;
    expect(tree.value).toContain('RewardStandardQualitySuper');
    fireEvent.input(tree, {
      target: {
        value: JSON.stringify({
          tag: 'thingSetMakerTags',
          attrs: [],
          children: [{ tag: 'li', attrs: [], children: ['RewardStandardQualityLow'] }],
        }),
      },
    });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Save field' }));
    expect(env.setField).toHaveBeenCalledWith(
      `/extraFields/${index}`,
      expect.objectContaining({ tag: 'thingSetMakerTags' }),
    );
  });

  it('removes one field, and removes the whole list when it was the last', () => {
    const env = makeEnv({ spec });
    const index = fields.findIndex((n) => n.tag === 'possessionCount');
    renderWithProviders(
      <WithEnv env={env}>
        <RawNodeList pointer="/extraFields" label="Other definition fields" />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Remove possessionCount' }));
    expect(env.setField).toHaveBeenCalledWith(`/extraFields/${index}`, undefined);

    const single = makeEnv({
      spec: { ...spec, extraFields: [{ tag: 'a', attrs: [], children: ['1'] }] },
    });
    renderWithProviders(
      <WithEnv env={single}>
        <RawNodeList pointer="/extraFields" label="Last field" />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Remove a' }));
    expect(single.setField).toHaveBeenCalledWith('/extraFields', undefined);
  });

  it('adds a new entry at the end, starting with the given tag', () => {
    const env = makeEnv({ spec: { ...spec, comps: undefined } });
    renderWithProviders(
      <WithEnv env={env}>
        <RawNodeList pointer="/comps" label="Components" defaultTag="li" />
      </WithEnv>,
    );
    expect(screen.getByText('None.')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Add field' }));
    const dialog = screen.getByRole('dialog');
    expect((within(dialog).getByLabelText(/Tag/) as HTMLInputElement).value).toBe('li');
    fireEvent.input(within(dialog).getByLabelText('Text'), { target: { value: 'x' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Save field' }));
    expect(env.setField).toHaveBeenCalledWith('/comps/0', {
      tag: 'li',
      attrs: [],
      children: ['x'],
    });
  });

  it('shows what the backend says about an entry under that entry', () => {
    const bad = fixture<PreviewDto>('designer-fields-preview-bad-extra');
    const draft = fixture<{ spec: typeof spec }>('designer-fields-draft-bad-extra');
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: draft.spec, diagnostics: bad.diagnostics })}>
        <RawNodeList pointer="/extraFields" label="Other definition fields" />
      </WithEnv>,
    );
    expect(screen.getByText(/also written from an input of the designer/)).toBeTruthy();
  });
});
