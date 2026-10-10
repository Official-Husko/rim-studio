#!/usr/bin/env node
// Draws the roadmap picture, docs/roadmap.svg, from the state file docs/roadmap-status.json.
//
//   node scripts/roadmap-svg.mjs           write docs/roadmap.svg
//   node scripts/roadmap-svg.mjs --check   fail when the picture is not what the state file produces
//
// The plan itself is docs/roadmap.md. The state file only records how far each scope item of a
// milestone has come (done, partial, open or deferred) and is edited by hand when the state changes.
// The picture is derived and deterministic: the same state file always gives the same bytes.
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const dataPath = join(root, 'docs', 'roadmap-status.json');
const svgPath = join(root, 'docs', 'roadmap.svg');
const planPath = join(root, 'docs', 'roadmap.md');
const checkOnly = process.argv.includes('--check');

const STATES = ['done', 'partial', 'open', 'deferred'];
const WEIGHT = { done: 1, partial: 0.5, open: 0 };

function fail(message) {
  process.stderr.write(`roadmap: ${message}\n`);
  process.exit(1);
}

// ---------- the data ----------

function loadData() {
  let data;
  try {
    data = JSON.parse(readFileSync(dataPath, 'utf8'));
  } catch (error) {
    fail(`cannot read docs/roadmap-status.json: ${error.message}`);
  }
  const need = (condition, message) => {
    if (!condition) fail(message);
  };
  need(/^\d{4}-\d{2}-\d{2}$/.test(data.updated ?? ''), '"updated" must be a date like 2026-10-10');
  const checkItems = (items, where) => {
    need(Array.isArray(items) && items.length > 0, `${where} needs a non empty list`);
    for (const item of items) {
      need(typeof item.text === 'string' && item.text.length > 0, `${where}: an item has no text`);
      need(STATES.includes(item.state), `${where}: "${item.text}" has the state "${item.state}"`);
    }
  };
  need(data.release && typeof data.release.id === 'string', 'the release block is missing');
  checkItems(data.release.scope, 'release.scope');
  checkItems(data.release.exit, 'release.exit');
  checkItems(data.release.extras, 'release.extras');
  need(Array.isArray(data.milestones) && data.milestones.length > 0, 'no milestones');
  const ids = new Set(data.milestones.map((m) => m.id));
  need(ids.size === data.milestones.length, 'a milestone id is used twice');
  for (const m of data.milestones) {
    need(/^M\d$/.test(m.id), `milestone id "${m.id}" must look like M3`);
    need(Number.isFinite(m.weeks) && m.weeks > 0, `${m.id}: weeks must be a positive number`);
    need(Array.isArray(m.needs) && m.needs.every((n) => ids.has(n)), `${m.id}: "needs" names an unknown milestone`);
    checkItems(m.items, m.id);
  }
  // every milestone of the picture is a milestone of the plan
  const plan = readFileSync(planPath, 'utf8');
  const planned = new Set([...plan.matchAll(/^## \d+\. (M\d) /gm)].map((x) => x[1]));
  for (const m of data.milestones) {
    need(planned.has(m.id), `${m.id} has no heading in docs/roadmap.md`);
  }
  for (const id of planned) {
    need(ids.has(id), `${id} is in docs/roadmap.md but not in docs/roadmap-status.json`);
  }
  return data;
}

function progressOf(items) {
  const counted = items.filter((i) => i.state !== 'deferred');
  if (counted.length === 0) return 1;
  return counted.reduce((sum, i) => sum + WEIGHT[i.state], 0) / counted.length;
}

function stateOf(items) {
  const counted = items.filter((i) => i.state !== 'deferred');
  if (counted.every((i) => i.state === 'done')) return 'done';
  if (counted.every((i) => i.state === 'open')) return 'open';
  return 'partial';
}

function countOf(items) {
  const counts = { done: 0, partial: 0, open: 0, deferred: 0 };
  for (const i of items) counts[i.state] += 1;
  return counts;
}

const percent = (fraction) => `${Math.round(fraction * 100)}%`;

// ---------- drawing helpers ----------

const W = 1200;
const M = 32;
const INNER = W - 2 * M;
const FONT = "system-ui, -apple-system, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif";
const CHAR = 0.58; // average glyph width as a fraction of the font size, kept on the wide side

const C = {
  bg: '#081a31',
  panel: '#0d2440',
  line: '#1d4570',
  text: '#e8f2ff',
  muted: '#9bb8d6',
  faint: '#6a8aab',
  track: '#163457',
  accent: '#4cc9f0',
  done: '#3ddc97',
  partial: '#ffc857',
  open: '#6a8aab',
  deferred: '#b48ead',
};

const esc = (s) =>
  String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

function attrs(map) {
  return Object.entries(map)
    .filter(([, v]) => v !== undefined)
    .map(([k, v]) => `${k}="${esc(v)}"`)
    .join(' ');
}

function text(x, y, content, { size = 13, weight = 400, fill = C.text, anchor, maxWidth } = {}) {
  if (maxWidth !== undefined && String(content).length * size * CHAR > maxWidth) {
    fail(`text too long for its box (${maxWidth}px at ${size}px): "${content}"`);
  }
  const a = attrs({
    x,
    y,
    'font-size': size,
    'font-weight': weight === 400 ? undefined : weight,
    fill,
    'text-anchor': anchor,
  });
  return `<text ${a}>${esc(content)}</text>`;
}

function rect(x, y, w, h, { fill = 'none', stroke, strokeWidth, rx, dash } = {}) {
  const a = attrs({
    x,
    y,
    width: w,
    height: h,
    rx,
    fill,
    stroke,
    'stroke-width': strokeWidth,
    'stroke-dasharray': dash,
  });
  return `<rect ${a}/>`;
}

function glyph(state, cx, cy) {
  const r = 6;
  switch (state) {
    case 'done':
      return [
        `<circle cx="${cx}" cy="${cy}" r="${r}" fill="${C.done}"/>`,
        `<path d="M${cx - 3},${cy} L${cx - 0.8},${cy + 2.6} L${cx + 3.4},${cy - 2.6}" fill="none" stroke="${C.bg}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>`,
      ].join('');
    case 'partial':
      return [
        `<circle cx="${cx}" cy="${cy}" r="${r - 0.8}" fill="none" stroke="${C.partial}" stroke-width="1.6"/>`,
        `<path d="M${cx},${cy - r + 0.8} A${r - 0.8},${r - 0.8} 0 0,0 ${cx},${cy + r - 0.8} Z" fill="${C.partial}"/>`,
      ].join('');
    case 'deferred':
      return `<rect x="${cx - 5}" y="${cy - 5}" width="10" height="10" rx="2" fill="none" stroke="${C.deferred}" stroke-width="1.5" stroke-dasharray="2.5 2"/>`;
    default:
      return `<circle cx="${cx}" cy="${cy}" r="${r - 0.8}" fill="none" stroke="${C.open}" stroke-width="1.6"/>`;
  }
}

function bar(x, y, w, h, fraction, color) {
  const filled = Math.round(w * fraction * 10) / 10;
  return [
    rect(x, y, w, h, { fill: C.track, rx: h / 2 }),
    filled > 0 ? rect(x, y, filled, h, { fill: color, rx: h / 2 }) : '',
  ].join('');
}

const colorOf = (state) => C[state] ?? C.open;

// ---------- the picture ----------

function draw(data) {
  const body = [];
  const milestones = data.milestones.map((m) => ({
    ...m,
    progress: progressOf(m.items),
    state: stateOf(m.items),
    counts: countOf(m.items),
  }));
  const totalWeeks = milestones.reduce((s, m) => s + m.weeks, 0);
  const overall = milestones.reduce((s, m) => s + m.progress * m.weeks, 0) / totalWeeks;
  const rel = data.release;
  const relCheck = [...rel.exit];
  const relProgress = progressOf(relCheck);
  const relCounts = countOf(relCheck);

  // header
  body.push(text(M, 60, 'RimStudio roadmap', { size: 32, weight: 700 }));
  body.push(
    text(M, 90, `Status of ${data.updated}. Plan: docs/roadmap.md. Data: docs/roadmap-status.json.`, {
      size: 14,
      fill: C.muted,
      maxWidth: 800,
    }),
  );
  body.push(text(W - M, 62, percent(overall), { size: 44, weight: 700, fill: C.accent, anchor: 'end' }));
  body.push(text(W - M, 86, 'of the planned effort is done', { size: 13, fill: C.muted, anchor: 'end' }));
  body.push(text(W - M, 104, `weighted by size, ${totalWeeks} person-weeks in total`, { size: 12, fill: C.faint, anchor: 'end' }));

  // legend
  let lx = M;
  const legend = [
    ['done', 'done'],
    ['partial', 'partly done or built as a temporary version'],
    ['open', 'not started'],
  ];
  if (milestones.some((m) => m.items.some((i) => i.state === 'deferred')) || [...rel.scope, ...rel.exit].some((i) => i.state === 'deferred')) {
    legend.push(['deferred', 'deferred']);
  }
  for (const [state, label] of legend) {
    body.push(glyph(state, lx + 7, 124));
    body.push(text(lx + 20, 129, label, { size: 12, fill: C.muted }));
    lx += 30 + label.length * 12 * CHAR + 16;
  }
  body.push(rect(lx, 115, 44, 18, { stroke: C.accent, strokeWidth: 1.2, rx: 9 }));
  body.push(text(lx + 22, 128, rel.id, { size: 11, weight: 600, fill: C.accent, anchor: 'middle' }));
  body.push(text(lx + 54, 129, `part of the ${rel.id} slice`, { size: 12, fill: C.muted }));

  // release panel: heights are computed first so that the box can be drawn before its contents
  const relTop = 152;
  const colW = 540;
  const leftX = M + 24;
  const rightX = M + 24 + colW + 28;
  const rowH = 24;
  const openItems = [...rel.scope, ...rel.exit].filter((i) => i.state === 'partial' || i.state === 'open');
  const stillOpen = openItems.length > 0 ? openItems : [{ text: 'Nothing: every check is met', state: 'done' }];
  const blockH = (items) => 20 + items.length * rowH;
  const contentTop = relTop + 106;
  const leftH = blockH(rel.scope) + 14 + blockH(rel.exit);
  const rightH = blockH(rel.extras) + 14 + blockH(stillOpen);
  const relBottom = contentTop + Math.max(leftH, rightH) + 4;

  body.push(rect(M, relTop, INNER, relBottom - relTop, { fill: C.panel, stroke: C.accent, strokeWidth: 1.5, rx: 10 }));
  body.push(text(M + 24, relTop + 40, `Release ${rel.id}: ${rel.title}`, { size: 22, weight: 700 }));
  body.push(text(M + 24, relTop + 66, rel.summary, { size: 13, fill: C.muted, maxWidth: 740 }));
  body.push(bar(W - M - 24 - 300, relTop + 28, 300, 12, relProgress, C.accent));
  body.push(
    text(
      W - M - 24,
      relTop + 62,
      `${percent(relProgress)}: ${relCounts.done} of ${relCheck.length} checks met, ${relCounts.partial} partly, ${relCounts.open} open`,
      { size: 13, fill: C.muted, anchor: 'end' },
    ),
  );
  const column = (x, y, heading, items) => {
    body.push(text(x, y, heading.toUpperCase(), { size: 12, weight: 700, fill: C.muted }));
    let yy = y + 20;
    for (const item of items) {
      body.push(glyph(item.state, x + 7, yy - 4));
      body.push(text(x + 24, yy, item.text, { size: 13, maxWidth: colW - 24 }));
      yy += rowH;
    }
    return yy;
  };
  column(leftX, column(leftX, contentTop, 'Scope', rel.scope) + 14, 'Exit criteria', rel.exit);
  column(rightX, column(rightX, contentTop, 'Built beyond the plan', rel.extras) + 14, `Still open for ${rel.id}`, stillOpen);

  // flow
  const flowTop = relBottom + 36;
  body.push(text(M, flowTop + 20, 'Milestones and their order', { size: 18, weight: 700 }));
  const chipW = 208;
  const chipH = 78;
  const gapX = 24;
  const gapY = 22;
  const grid = { M0: [0, 0], M1: [1, 0], M2: [2, 0], M3: [3, 0], M7: [4, 0], M4: [2, 1], M5: [3, 1], M6: [3, 2] };
  const chipsTop = flowTop + 44;
  const pos = {};
  for (const m of milestones) {
    const cell = grid[m.id];
    if (!cell) fail(`no place for ${m.id} in the flow drawing`);
    pos[m.id] = { x: M + cell[0] * (chipW + gapX), y: chipsTop + cell[1] * (chipH + gapY) };
  }
  const flowBottom = chipsTop + 3 * chipH + 2 * gapY;
  const edgeColor = '#4a77a8';
  const edge = (d) => `<path d="${d}" fill="none" stroke="${edgeColor}" stroke-width="2" marker-end="url(#arrow)"/>`;
  const right = (id) => ({ x: pos[id].x + chipW, y: pos[id].y + chipH / 2 });
  const left = (id) => ({ x: pos[id].x, y: pos[id].y + chipH / 2 });
  const bottom = (id) => ({ x: pos[id].x + chipW / 2, y: pos[id].y + chipH });
  const top = (id) => ({ x: pos[id].x + chipW / 2, y: pos[id].y });
  const edges = [];
  const straight = (a, b) => edges.push(edge(`M${a.x},${a.y} L${b.x - 2},${b.y}`));
  straight(right('M0'), left('M1'));
  straight(right('M1'), left('M2'));
  straight(right('M2'), left('M3'));
  straight(right('M3'), left('M7'));
  edges.push(edge(`M${bottom('M1').x},${bottom('M1').y} V${left('M4').y} H${left('M4').x - 2}`));
  edges.push(edge(`M${bottom('M2').x},${bottom('M2').y} V${top('M4').y - 2}`));
  straight(right('M4'), left('M5'));
  edges.push(edge(`M${bottom('M4').x},${bottom('M4').y} V${left('M6').y} H${left('M6').x - 2}`));
  const mergeX = pos.M7.x + chipW / 2;
  edges.push(edge(`M${right('M5').x},${right('M5').y} H${mergeX} V${bottom('M7').y + 2}`));
  edges.push(`<path d="M${right('M6').x},${right('M6').y} H${mergeX} V${right('M5').y}" fill="none" stroke="${edgeColor}" stroke-width="2"/>`);
  body.push(...edges);
  for (const m of milestones) {
    const { x, y } = pos[m.id];
    const stateColor = colorOf(m.state === 'done' ? 'done' : m.state === 'open' ? 'open' : 'partial');
    body.push(rect(x, y, chipW, chipH, { fill: C.panel, stroke: C.line, strokeWidth: 1.5, rx: 8 }));
    body.push(rect(x, y + 8, 5, chipH - 16, { fill: stateColor, rx: 2.5 }));
    body.push(text(x + 18, y + 28, m.id, { size: 18, weight: 700 }));
    body.push(text(x + 58, y + 27, `${m.weeks} wk`, { size: 12, fill: C.faint }));
    if (m.slice) {
      body.push(rect(x + chipW - 54, y + 10, 44, 18, { stroke: C.accent, strokeWidth: 1.2, rx: 9 }));
      body.push(text(x + chipW - 32, y + 23, rel.id, { size: 11, weight: 600, fill: C.accent, anchor: 'middle' }));
    }
    body.push(text(x + 18, y + 48, m.title, { size: 14, maxWidth: chipW - 28 }));
    body.push(bar(x + 18, y + 59, 140, 8, m.progress, stateColor));
    body.push(text(x + chipW - 12, y + 67, percent(m.progress), { size: 13, weight: 700, fill: stateColor, anchor: 'end' }));
  }
  body.push(
    text(M, chipsTop + chipH + gapY + 28, `${rel.id} takes the parts it needs from M0, M1, M4 and M5`, { size: 13, fill: C.muted }),
    text(M, chipsTop + chipH + gapY + 48, 'and finishes the weapons half of the designer first.', { size: 13, fill: C.muted }),
    text(M, chipsTop + chipH + gapY + 80, 'M6 needs M4 only, so it can move ahead of M5.', { size: 13, fill: C.muted }),
  );

  // detail bands
  let y = flowBottom + 40;
  body.push(text(M, y + 4, 'Scope of each milestone', { size: 18, weight: 700 }));
  y += 24;
  const c1 = M + 300;
  const c2 = M + 300 + 420;
  const itemW = 420;
  for (const m of milestones) {
    const rows = Math.ceil(m.items.length / 2);
    const bandH = Math.max(132, rows * rowH + 40);
    const stateColor = colorOf(m.state === 'done' ? 'done' : m.state === 'open' ? 'open' : 'partial');
    body.push(rect(M, y, INNER, bandH, { fill: C.panel, stroke: C.line, strokeWidth: 1.5, rx: 10 }));
    body.push(rect(M, y + 10, 5, bandH - 20, { fill: stateColor, rx: 2.5 }));
    body.push(text(M + 24, y + 36, m.id, { size: 24, weight: 700, fill: stateColor }));
    body.push(text(M + 24, y + 62, m.title, { size: 16, weight: 600, maxWidth: 250 }));
    body.push(text(M + 24, y + 84, `${m.weeks} person-weeks`, { size: 12, fill: C.muted }));
    body.push(text(M + 24, y + 102, m.needs.length > 0 ? `needs ${m.needs.join(', ')}` : 'needs nothing', { size: 12, fill: C.muted }));
    body.push(bar(M + 24, y + 114, 190, 8, m.progress, stateColor));
    body.push(text(M + 224, y + 123, percent(m.progress), { size: 13, weight: 700, fill: stateColor }));
    m.items.forEach((item, i) => {
      const col = i < rows ? 0 : 1;
      const row = col === 0 ? i : i - rows;
      const x = col === 0 ? c1 : c2;
      const yy = y + 34 + row * rowH;
      body.push(glyph(item.state, x + 7, yy - 4));
      body.push(text(x + 24, yy, item.text, { size: 13, maxWidth: itemW - 24 }));
    });
    y += bandH + 16;
  }

  // footer
  body.push(
    text(M, y + 14, 'Generated file. Edit docs/roadmap-status.json and run pnpm roadmap; pnpm roadmap:check fails when this picture is stale.', {
      size: 12,
      fill: C.faint,
    }),
  );
  const H = y + 40;

  const summary = `Roadmap status of ${data.updated}: ${percent(overall)} of the planned effort is done. ${milestones
    .map((m) => `${m.id} ${m.title} ${percent(m.progress)}`)
    .join(', ')}. Release ${rel.id}: ${relCounts.done} of ${relCheck.length} checks met.`;
  const head = [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" role="img" aria-labelledby="roadmap-title roadmap-desc" font-family="${esc(FONT)}">`,
    `<title id="roadmap-title">RimStudio roadmap</title>`,
    `<desc id="roadmap-desc">${esc(summary)}</desc>`,
    `<defs><marker id="arrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 Z" fill="${edgeColor}"/></marker></defs>`,
    `<rect width="${W}" height="${H}" fill="${C.bg}"/>`,
  ];
  return `${head.join('\n')}\n${body.join('\n')}\n</svg>\n`;
}

const svg = draw(loadData());
if (checkOnly) {
  let current = '';
  try {
    current = readFileSync(svgPath, 'utf8');
  } catch {
    /* missing file counts as stale */
  }
  if (current !== svg) fail('docs/roadmap.svg is stale: run `pnpm roadmap` and commit the result.');
  process.stdout.write('roadmap: docs/roadmap.svg is up to date\n');
} else {
  writeFileSync(svgPath, svg);
  process.stdout.write(`roadmap: wrote docs/roadmap.svg (${svg.length} bytes)\n`);
}
