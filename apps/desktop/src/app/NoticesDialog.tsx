import type { ComponentChildren } from 'preact';
import { Button, Dialog } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { openUrl } from '~/shared/platform';
// The notices file of the repository: the attribution that goes with the included game data.
import noticeSource from '../../../../NOTICE.md?raw';

/** One block of the notice: a heading, a paragraph or a table. */
export type NoticeBlock =
  | { kind: 'heading'; level: 1 | 2; text: string }
  | { kind: 'paragraph'; text: string }
  | { kind: 'table'; head: string[]; rows: string[][] };

const cells = (line: string): string[] =>
  line
    .trim()
    .replace(/^\|/, '')
    .replace(/\|$/, '')
    .split('|')
    .map((c) => c.trim());

/** Reads the small Markdown subset that NOTICE.md uses: headings, paragraphs and pipe tables. */
export function parseNotice(source: string): NoticeBlock[] {
  const blocks: NoticeBlock[] = [];
  let paragraph: string[] = [];
  let table: string[][] = [];
  const flushParagraph = (): void => {
    if (paragraph.length > 0) blocks.push({ kind: 'paragraph', text: paragraph.join(' ') });
    paragraph = [];
  };
  const flushTable = (): void => {
    const [head, ...rows] = table;
    if (head) blocks.push({ kind: 'table', head, rows });
    table = [];
  };
  for (const raw of source.split('\n')) {
    const line = raw.trim();
    if (line.startsWith('|')) {
      flushParagraph();
      if (!/^\|[\s|:-]+\|$/.test(line)) table.push(cells(line));
      continue;
    }
    flushTable();
    const heading = /^(#{1,2})\s+(.*)$/.exec(line);
    if (heading) {
      flushParagraph();
      blocks.push({
        kind: 'heading',
        level: heading[1]?.length === 1 ? 1 : 2,
        text: heading[2] ?? '',
      });
    } else if (line === '') {
      flushParagraph();
    } else {
      paragraph.push(line);
    }
  }
  flushParagraph();
  flushTable();
  return blocks;
}

const INLINE = /(\[[^\]]+\]\([^)]+\))|(https?:\/\/[^\s)`]*[^\s).,;`])|(`[^`]+`)/g;

/** Text with links and code spans. Web addresses open through the platform opener. */
export function Inline({ text }: { text: string }) {
  const out: ComponentChildren[] = [];
  let last = 0;
  for (const match of text.matchAll(INLINE)) {
    const at = match.index ?? 0;
    if (at > last) out.push(text.slice(last, at));
    const [whole, markdown, url, code] = match;
    if (markdown) {
      const parts = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(markdown);
      const label = parts?.[1] ?? markdown;
      const target = parts?.[2] ?? '';
      out.push(/^https?:\/\//.test(target) ? <Link url={target} label={label} /> : label);
    } else if (url) {
      out.push(<Link url={url} label={url} />);
    } else if (code) {
      out.push(<code class="font-mono text-mono-small">{code.slice(1, -1)}</code>);
    }
    last = at + whole.length;
  }
  if (last < text.length) out.push(text.slice(last));
  return <>{out}</>;
}

function Link({ url, label }: { url: string; label: string }) {
  return (
    <a
      href={url}
      target="_blank"
      rel="noreferrer"
      class="text-accent underline underline-offset-2"
      onClick={(e) => {
        e.preventDefault();
        openUrl(url);
      }}
    >
      {label}
    </a>
  );
}

export interface NoticesDialogProps {
  open: boolean;
  onClose: () => void;
  /** The notice text; the file NOTICE.md of the repository by default. */
  source?: string;
}

/** The notices of the project: who made the games whose data is included, and under which terms. */
export function NoticesDialog({ open, onClose, source = noticeSource }: NoticesDialogProps) {
  const blocks = parseNotice(source).filter((b) => !(b.kind === 'heading' && b.level === 1));
  return (
    <Dialog
      open={open}
      size="lg"
      title={t('app.notices.title')}
      closeLabel={t('app.notices.close')}
      onClose={onClose}
      footer={<Button onClick={onClose}>{t('app.notices.close')}</Button>}
    >
      <div class="flex max-h-[60vh] flex-col gap-3 overflow-auto pr-2 text-body">
        {blocks.map((block, i) => {
          if (block.kind === 'heading') {
            return (
              <h3 key={i} class="m-0 mt-2 text-title font-semibold">
                {block.text}
              </h3>
            );
          }
          if (block.kind === 'paragraph') {
            return (
              <p key={i} class="m-0 text-muted">
                <Inline text={block.text} />
              </p>
            );
          }
          return (
            <div key={i} class="overflow-x-auto">
              <table class="w-full border-collapse text-small">
                <thead>
                  <tr class="text-left text-muted">
                    {block.head.map((h) => (
                      <th key={h} scope="col" class="py-1 pr-3 font-semibold">
                        {h}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {block.rows.map((row) => (
                    <tr key={row.join('|')} class="border-t border-line-subtle align-top">
                      {row.map((cell, c) => (
                        <td key={c} class="py-1 pr-3">
                          <Inline text={cell} />
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          );
        })}
      </div>
    </Dialog>
  );
}
