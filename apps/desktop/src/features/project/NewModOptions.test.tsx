import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { NewModOptions } from './NewModOptions';
import { emptyForm } from './scaffold';

describe('NewModOptions', () => {
  it('starts with the documented defaults', () => {
    renderWithProviders(<NewModOptions form={emptyForm()} onChange={vi.fn()} />);
    expect((screen.getByLabelText('Patches') as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText('Textures') as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText('Sounds') as HTMLInputElement).checked).toBe(true);
    expect(
      (screen.getByLabelText('Combat Extended patch folder, gated') as HTMLInputElement).checked,
    ).toBe(false);
  });

  it('reports a change of one option', () => {
    const onChange = vi.fn();
    renderWithProviders(<NewModOptions form={emptyForm()} onChange={onChange} />);
    fireEvent.click(screen.getByLabelText('Combat Extended patch folder, gated'));
    expect(onChange).toHaveBeenCalledWith({ cePatchFolder: true });
  });

  it('keeps the source art ignore option off until there is a source folder', () => {
    const { rerender } = renderWithProviders(
      <NewModOptions form={emptyForm()} onChange={vi.fn()} />,
    );
    const ignore = screen.getByLabelText(
      'Keep Source art out of the repository',
    ) as HTMLInputElement;
    expect(ignore.disabled).toBe(true);
    rerender(<NewModOptions form={{ ...emptyForm(), sourceFolder: true }} onChange={vi.fn()} />);
    expect(ignore.disabled).toBe(false);
  });
});
