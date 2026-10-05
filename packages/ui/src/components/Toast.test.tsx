import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { Toast, ToastStack } from './Toast';

describe('Toast', () => {
  it('announces errors as alerts and information as status', () => {
    render(
      <ToastStack>
        <Toast tone="error" message="Write failed" />
        <Toast tone="success" message="Files written" />
      </ToastStack>,
    );
    expect(screen.getByRole('alert').textContent).toContain('Write failed');
    expect(screen.getByRole('status').textContent).toContain('Files written');
  });

  it('has one action and a dismiss button', () => {
    const onAction = vi.fn();
    const onDismiss = vi.fn();
    render(
      <Toast
        message="Draft deleted"
        actionLabel="Undo"
        onAction={onAction}
        onDismiss={onDismiss}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(onAction).toHaveBeenCalled();
    expect(onDismiss).toHaveBeenCalled();
  });

  it('puts the stack in a labelled region', () => {
    render(<ToastStack label="Messages" />);
    expect(screen.getByRole('region', { name: 'Messages' })).toBeTruthy();
  });
});
