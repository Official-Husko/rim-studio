import { fireEvent, render, screen, waitFor } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { CodeView } from './CodeView';
import { tokenizeXml } from './xmlTokens';

const SAMPLE = `<?xml version="1.0" encoding="utf-8"?>
<Defs>
  <!-- a comment
       over two lines -->
  <ThingDef ParentName="BaseGun">
    <defName>Gun_Carbine</defName>
    <label>carbine</label>
  </ThingDef>
</Defs>`;

describe('tokenizeXml', () => {
  it('gives back the source text line by line', () => {
    const lines = tokenizeXml(SAMPLE).map((l) => l.map((t) => t.text).join(''));
    expect(lines.join('\n')).toBe(SAMPLE);
  });

  it('classifies tags, attributes and values', () => {
    const [, defs, , , thing] = tokenizeXml(SAMPLE);
    expect(defs?.find((t) => t.kind === 'tag')?.text).toBe('Defs');
    const kinds = thing?.map((t) => `${t.kind}:${t.text}`) ?? [];
    expect(kinds).toContain('attr:ParentName');
    expect(kinds).toContain('value:"BaseGun"');
  });

  it('keeps a multi line comment as comment tokens', () => {
    const lines = tokenizeXml(SAMPLE);
    expect(lines[3]?.every((t) => t.kind === 'comment' || /^\s+$/.test(t.text))).toBe(true);
  });

  it('survives broken input', () => {
    const broken = '<a href="x';
    expect(
      tokenizeXml(broken)
        .flat()
        .map((t) => t.text)
        .join(''),
    ).toBe(broken);
    expect(tokenizeXml('')).toEqual([[]]);
  });
});

describe('CodeView', () => {
  it('shows numbered lines in a labelled region', () => {
    render(<CodeView label="Definition" code={SAMPLE} />);
    expect(screen.getByRole('region', { name: 'Definition' })).toBeTruthy();
    expect(screen.getByText('9')).toBeTruthy();
  });

  it('can hide line numbers', () => {
    const { container } = render(
      <CodeView label="Plain" code={'a\nb'} language="text" lineNumbers={false} />,
    );
    expect(container.querySelector('[data-gutter]')).toBeNull();
  });

  it('copies the code and reports it', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    const onCopy = vi.fn();
    render(<CodeView label="Definition" code="<a/>" onCopy={onCopy} />);
    fireEvent.click(screen.getByRole('button', { name: 'Copy' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Copied' })).toBeTruthy());
    expect(writeText).toHaveBeenCalledWith('<a/>');
    expect(onCopy).toHaveBeenCalled();
  });
});
