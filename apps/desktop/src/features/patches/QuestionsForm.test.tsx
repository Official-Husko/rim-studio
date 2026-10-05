import { fireEvent, render, screen, waitFor } from '@testing-library/preact';
import { loadFixture } from 'rimstudio-testkit';
import type { ConvertScanDto } from 'rimstudio-ipc-types';
import { beforeEach, describe, expect, it } from 'vitest';
import { familyAnswers, ownAnswers } from './answerStore';
import { QuestionsForm } from './QuestionsForm';
import { installTransport } from './testSupport';

const candidate = loadFixture<ConvertScanDto>('designer_convert_scan').candidates[0];
if (!candidate) throw new Error('fixture');

beforeEach(() => {
  installTransport();
});

describe('QuestionsForm', () => {
  it('counts the answers and stores a number for the weapon', async () => {
    render(<QuestionsForm candidate={candidate} familySize={1} />);
    expect(screen.getByText('0 of 9 answered')).toBeTruthy();
    expect(screen.queryByRole('radiogroup', { name: 'Apply answers to' })).toBeNull();
    const spread = screen.getByRole('spinbutton', { name: 'CE shot spread' }) as HTMLInputElement;
    fireEvent.input(spread, { target: { value: '0.1' } });
    fireEvent.blur(spread);
    await waitFor(() => expect(ownAnswers.value['OH_G41m']?.numbers['/ce/shotSpread']).toBe(0.1));
    expect(await screen.findByText('1 of 9 answered')).toBeTruthy();
  });

  it('writes to the family when the family scope is chosen', async () => {
    render(<QuestionsForm candidate={candidate} familySize={3} />);
    fireEvent.click(screen.getByRole('radio', { name: 'Its family (3 weapons)' }));
    fireEvent.click(within9('Is the weapon belt fed?', 'Yes'));
    await waitFor(() => expect(familyAnswers.value[candidate.family ?? '']?.beltFed).toBe(true));
    expect(ownAnswers.value['OH_G41m']).toBeUndefined();
    expect(await screen.findByText('1 of 9 answered')).toBeTruthy();
  });

  it('says so when nothing is asked', () => {
    render(<QuestionsForm candidate={{ ...candidate, asks: [] }} familySize={1} />);
    expect(screen.getByText(/Nothing is left to ask/)).toBeTruthy();
  });
});

function within9(group: string, name: string): HTMLElement {
  const g = screen.getByRole('radiogroup', { name: group });
  return Array.from(g.querySelectorAll('[role="radio"]')).find(
    (r) => r.textContent === name,
  ) as HTMLElement;
}
