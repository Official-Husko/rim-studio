import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { AttachmentLinkRow } from './AttachmentLinkRow';

describe('AttachmentLinkRow', () => {
  it('edits the name and drops an emptied draw scale', () => {
    const onChange = vi.fn();
    render(
      <AttachmentLinkRow
        link={{ attachment: 'RS_Scope', drawScale: '(0.5,0.5)' }}
        onChange={onChange}
        onRemove={() => undefined}
      />,
    );
    fireEvent.input(screen.getByRole('textbox', { name: 'Attachment definition' }), {
      target: { value: 'RS_Rail' },
    });
    expect(onChange).toHaveBeenLastCalledWith({ attachment: 'RS_Rail', drawScale: '(0.5,0.5)' });
    fireEvent.input(screen.getByRole('textbox', { name: 'Draw scale' }), { target: { value: '' } });
    expect(onChange).toHaveBeenLastCalledWith({ attachment: 'RS_Scope' });
  });

  it('adds a stat offset and reports a removal of the link', () => {
    const onChange = vi.fn();
    const onRemove = vi.fn();
    render(
      <AttachmentLinkRow
        link={{ attachment: 'RS_Scope' }}
        onChange={onChange}
        onRemove={onRemove}
      />,
    );
    const group = screen.getByRole('group', { name: 'Stat offsets' });
    fireEvent.click(group.querySelector('button') as HTMLButtonElement);
    expect(onChange).toHaveBeenLastCalledWith({
      attachment: 'RS_Scope',
      statOffsets: [{ stat: '', value: 0 }],
    });
    fireEvent.click(screen.getByRole('button', { name: 'Remove attachment' }));
    expect(onRemove).toHaveBeenCalled();
  });
});
