import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { UnderBarrelFireModes } from './UnderBarrelFireModes';

describe('UnderBarrelFireModes', () => {
  it('sets the AI burst choice and the no single shot flag', () => {
    const onChange = vi.fn();
    render(<UnderBarrelFireModes modes={{ noSingleShot: false }} onChange={onChange} />);
    fireEvent.click(screen.getByRole('radio', { name: 'Yes' }));
    expect(onChange).toHaveBeenLastCalledWith({ noSingleShot: false, aiUseBurstMode: true });
    fireEvent.click(screen.getByRole('switch', { name: /single shot/i }));
    expect(onChange).toHaveBeenLastCalledWith({ noSingleShot: true });
  });

  it('removes the aim mode when its text is emptied', () => {
    const onChange = vi.fn();
    render(
      <UnderBarrelFireModes
        modes={{ noSingleShot: false, aiAimMode: 'SnapShot' }}
        onChange={onChange}
      />,
    );
    fireEvent.input(screen.getByRole('textbox', { name: 'AI aim mode' }), {
      target: { value: '' },
    });
    expect(onChange).toHaveBeenLastCalledWith({ noSingleShot: false });
  });
});
