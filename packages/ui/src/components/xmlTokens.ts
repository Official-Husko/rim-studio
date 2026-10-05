/** A coloured span of source text. */
export type XmlTokenKind = 'tag' | 'attr' | 'value' | 'punct' | 'comment' | 'meta' | 'text';

export interface XmlToken {
  kind: XmlTokenKind;
  text: string;
}

const NAME_END = /[\s=/>]/;

/**
 * A small in house XML colouriser for display only: it never validates, never throws and
 * always returns the source text unchanged when the tokens are joined, line by line.
 */
export function tokenizeXml(source: string): XmlToken[][] {
  const flat: XmlToken[] = [];
  const push = (kind: XmlTokenKind, text: string): void => {
    if (text) flat.push({ kind, text });
  };
  let i = 0;
  const n = source.length;

  const readUntil = (end: string): string => {
    const at = source.indexOf(end, i);
    const stop = at < 0 ? n : at + end.length;
    const chunk = source.slice(i, stop);
    i = stop;
    return chunk;
  };

  while (i < n) {
    if (source.startsWith('<!--', i)) {
      push('comment', readUntil('-->'));
    } else if (source.startsWith('<![CDATA[', i)) {
      push('text', readUntil(']]>'));
    } else if (source.startsWith('<?', i)) {
      push('meta', readUntil('?>'));
    } else if (source.startsWith('<!', i)) {
      push('meta', readUntil('>'));
    } else if (source[i] === '<') {
      const close = source[i + 1] === '/';
      push('punct', close ? '</' : '<');
      i += close ? 2 : 1;
      let start = i;
      while (i < n && !NAME_END.test(source[i] ?? '')) i += 1;
      push('tag', source.slice(start, i));
      // attributes until the end of the tag
      while (i < n && source[i] !== '>') {
        const ch = source[i] ?? '';
        if (/\s/.test(ch)) {
          start = i;
          while (i < n && /\s/.test(source[i] ?? '')) i += 1;
          push('text', source.slice(start, i));
        } else if (ch === '/' || ch === '=') {
          push('punct', ch);
          i += 1;
        } else if (ch === '"' || ch === "'") {
          const endAt = source.indexOf(ch, i + 1);
          const stop = endAt < 0 ? n : endAt + 1;
          push('value', source.slice(i, stop));
          i = stop;
        } else {
          start = i;
          while (i < n && !NAME_END.test(source[i] ?? '')) i += 1;
          if (i === start) i += 1;
          push('attr', source.slice(start, i));
        }
      }
      if (i < n) {
        push('punct', '>');
        i += 1;
      }
    } else {
      const at = source.indexOf('<', i);
      const stop = at < 0 ? n : at;
      push('text', source.slice(i, stop));
      i = stop;
    }
  }

  // split into lines
  const lines: XmlToken[][] = [[]];
  for (const token of flat) {
    const parts = token.text.split('\n');
    parts.forEach((part, index) => {
      if (index > 0) lines.push([]);
      if (part) lines[lines.length - 1]?.push({ kind: token.kind, text: part });
    });
  }
  return lines;
}
