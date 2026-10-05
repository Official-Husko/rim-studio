import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { RawNodeDialog } from './RawNodeDialog';

const base = { open: true, title: 'Edit', defaultTag: 'li', onClose: () => undefined };

describe('RawNodeDialog', () => {
  it('writes a value element from a tag and a text', () => {
    const onSave = vi.fn();
    renderWithProviders(<RawNodeDialog {...base} defaultTag="" onSave={onSave} />);
    fireEvent.input(screen.getByLabelText(/Tag/), { target: { value: 'relicChance' } });
    fireEvent.input(screen.getByLabelText('Text'), { target: { value: '3' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save field' }));
    expect(onSave).toHaveBeenCalledWith({ tag: 'relicChance', attrs: [], children: ['3'] });
  });

  it('cannot save without a tag', () => {
    renderWithProviders(<RawNodeDialog {...base} defaultTag="" onSave={() => undefined} />);
    expect((screen.getByRole('button', { name: 'Save field' }) as HTMLButtonElement).disabled).toBe(
      true,
    );
  });

  it('opens a node with attributes as a tree and keeps them', () => {
    const onSave = vi.fn();
    const node = {
      tag: 'comp',
      attrs: [['Class', 'CompProps']] as Array<[string, string]>,
      children: [],
    };
    renderWithProviders(<RawNodeDialog {...base} initial={node} onSave={onSave} />);
    const tree = screen.getByLabelText('Node tree (JSON)') as HTMLTextAreaElement;
    expect(JSON.parse(tree.value)).toEqual(node);
    fireEvent.click(screen.getByRole('button', { name: 'Save field' }));
    expect(onSave).toHaveBeenCalledWith(node);
  });

  it('switches a value element to the tree editor', () => {
    renderWithProviders(<RawNodeDialog {...base} defaultTag="" onSave={() => undefined} />);
    fireEvent.input(screen.getByLabelText(/Tag/), { target: { value: 'a' } });
    fireEvent.input(screen.getByLabelText('Text'), { target: { value: 'b' } });
    fireEvent.click(screen.getByRole('button', { name: 'Edit as a tree' }));
    const tree = screen.getByLabelText('Node tree (JSON)') as HTMLTextAreaElement;
    expect(JSON.parse(tree.value)).toEqual({ tag: 'a', attrs: [], children: ['b'] });
  });

  it('refuses text that is not JSON or not a node, and does not call save', () => {
    const onSave = vi.fn();
    const node = {
      tag: 'a',
      attrs: [] as Array<[string, string]>,
      children: [{ tag: 'b', attrs: [] as Array<[string, string]>, children: [] }],
    };
    renderWithProviders(<RawNodeDialog {...base} initial={node} onSave={onSave} />);
    const tree = screen.getByLabelText('Node tree (JSON)');
    fireEvent.input(tree, { target: { value: '{ nope' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save field' }));
    expect(screen.getByText('This is not valid JSON.')).toBeTruthy();
    fireEvent.input(tree, { target: { value: '{"tag": 3}' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save field' }));
    expect(screen.getByText(/A node needs a tag/)).toBeTruthy();
    expect(onSave).not.toHaveBeenCalled();
  });
});
