import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { FormField } from './FormField';
import { TextField } from './TextField';

describe('FormField', () => {
  it('labels the control', () => {
    render(
      <FormField label="Label">
        <TextField value="" onValueChange={() => {}} />
      </FormField>,
    );
    expect(screen.getByLabelText('Label')).toBeTruthy();
  });

  it('links help text and error to the control', () => {
    render(
      <FormField label="defName" help="Letters, digits and underscores." error="Already used.">
        <TextField value="x" onValueChange={() => {}} />
      </FormField>,
    );
    const input = screen.getByLabelText('defName');
    const described = (input.getAttribute('aria-describedby') ?? '').split(' ');
    expect(described).toHaveLength(2);
    expect(input.getAttribute('aria-invalid')).toBe('true');
    expect(screen.getByRole('alert').textContent).toBe('Already used.');
  });

  it('marks required fields', () => {
    render(
      <FormField label="Name" required>
        <TextField value="" onValueChange={() => {}} />
      </FormField>,
    );
    expect(screen.getByLabelText(/Name/).hasAttribute('required')).toBe(true);
  });
});
