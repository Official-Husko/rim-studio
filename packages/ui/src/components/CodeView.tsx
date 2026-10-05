import { useEffect, useMemo, useRef, useState } from 'preact/hooks';
import { cx } from '../cx';
import { Button } from './Button';
import { tokenizeXml, type XmlToken, type XmlTokenKind } from './xmlTokens';

export interface CodeViewProps {
  code: string;
  /** xml gets in house colouring; text is plain. */
  language?: 'xml' | 'text';
  /** Accessible name of the code region. */
  label: string;
  lineNumbers?: boolean;
  copyLabel?: string;
  copiedLabel?: string;
  /** Called after a successful copy. */
  onCopy?: () => void;
  /** Static utility class limiting the height, such as max-h-96. */
  heightClass?: string;
}

const COLOURS: Record<XmlTokenKind, string> = {
  tag: 'text-accent',
  attr: 'text-rule-community',
  value: 'text-success',
  punct: 'text-faint',
  comment: 'text-faint italic',
  meta: 'text-info',
  text: 'text-fg',
};

function plainLines(code: string): XmlToken[][] {
  return code.split('\n').map((text) => (text ? [{ kind: 'text' as const, text }] : []));
}

/** A read only monospace code block with line numbers and a copy button. */
export function CodeView({
  code,
  language = 'xml',
  label,
  lineNumbers = true,
  copyLabel = 'Copy',
  copiedLabel = 'Copied',
  onCopy,
  heightClass,
}: CodeViewProps) {
  const lines = useMemo(
    () => (language === 'xml' ? tokenizeXml(code) : plainLines(code)),
    [code, language],
  );
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>();
  useEffect(() => () => clearTimeout(timer.current), []);
  const gutter = String(lines.length).length;

  const copy = async (): Promise<void> => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      onCopy?.();
      clearTimeout(timer.current);
      timer.current = setTimeout(() => setCopied(false), 1500);
    } catch {
      /* clipboard can be blocked; the text stays selectable */
    }
  };

  return (
    <div role="region" aria-label={label} class="border border-line bg-surface">
      <div class="flex h-8 items-center justify-between border-b border-line-subtle px-2">
        <span class="font-mono text-mono-small text-faint uppercase">{language}</span>
        <Button size="sm" variant="ghost" icon={copied ? 'check' : 'copy'} onClick={copy}>
          {copied ? copiedLabel : copyLabel}
        </Button>
      </div>
      <pre
        class={cx('m-0 overflow-auto py-2 font-mono text-mono leading-5', heightClass)}
        // A scrollable code region must be reachable by keyboard.
        // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
      >
        <code class="block min-w-max">
          {lines.map((tokens, index) => (
            <div key={index} class="flex px-3">
              {lineNumbers ? (
                <span
                  aria-hidden="true"
                  data-gutter
                  class="mr-3 shrink-0 text-right text-faint select-none"
                  style={`min-width:${gutter}ch`}
                >
                  {index + 1}
                </span>
              ) : null}
              <span class="whitespace-pre">
                {tokens.length === 0
                  ? ' '
                  : tokens.map((t, k) => (
                      <span key={k} data-kind={t.kind} class={COLOURS[t.kind]}>
                        {t.text}
                      </span>
                    ))}
              </span>
            </div>
          ))}
        </code>
      </pre>
    </div>
  );
}
