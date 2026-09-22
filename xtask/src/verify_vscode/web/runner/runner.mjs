import fs from 'node:fs/promises';
import net from 'node:net';
import path from 'node:path';
import { createRequire } from 'node:module';
import { open } from '@vscode/test-web';
import { chromium } from 'playwright';

const require = createRequire(import.meta.url);
const [workspace, results, cache] = process.argv.slice(2);
if (!workspace || !results || !cache) {
  throw new Error('usage: runner.mjs <workspace> <results> <cache>');
}

const options = JSON.parse(await fs.readFile(path.join(workspace, 'options.json'), 'utf8'));
if (!['side-by-side', 'inline'].includes(options.layout)) {
  throw new Error('layout must be side-by-side or inline');
}
if (typeof options.wrap !== 'boolean') {
  throw new Error('wrap must be boolean');
}
if (!Number.isInteger(options.wrap_column) || options.wrap_column < 1) {
  throw new Error('wrap_column must be a positive integer');
}

const manifestPath = require.resolve('@codediff/vscode-extension/package.json');
const extension = path.dirname(manifestPath);
const manifest = JSON.parse(await fs.readFile(path.join(extension, 'package.json'), 'utf8'));
const commit = manifest.vscodeCommit;

const pairs = (await fs.readFile(path.join(workspace, 'pairs.txt'), 'utf8'))
  .trim()
  .split('\n')
  .filter(Boolean)
  .map(line => {
    const [id, original, modified, originalLines, modifiedLines] = line.split('\t');
    return {
      id,
      original,
      modified,
      originalLines: Number(originalLines),
      modifiedLines: Number(modifiedLines),
    };
  });

const port = await freePort();
const server = await open({
  browserType: 'none',
  quality: 'stable',
  commit,
  extensionDevelopmentPath: extension,
  folderPath: workspace,
  host: 'localhost',
  port,
  headless: true,
  printServerLog: false,
  testRunnerDataDir: cache,
});

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({
  viewport: {
    width: viewportWidth(),
    height: await viewportHeight(pairs[0]),
  },
});

try {
  await page.goto(`http://localhost:${port}`);
  await fs.mkdir(results, { recursive: true });
  for (let index = 0; index < pairs.length; index++) {
    const pair = pairs[index];
    await page.setViewportSize({
      width: viewportWidth(),
      height: await viewportHeight(pair),
    });
    await page.getByText(`PARITY:${pair.id}`, { exact: true }).waitFor({ timeout: 60_000 });
    const diffEditor = options.layout === 'side-by-side'
      ? page.locator('.monaco-diff-editor.side-by-side')
      : page.locator('.monaco-diff-editor:not(.side-by-side)');
    await diffEditor.waitFor({ timeout: 60_000 });
    const modified = page.locator('.modified-in-monaco-diff-editor');
    await modified.waitFor({ state: 'visible', timeout: 60_000 });
    await modified.click({ position: { x: 100, y: 40 }, timeout: 60_000 });
    await page.keyboard.press(process.platform === 'darwin' ? 'Meta+ArrowUp' : 'Control+Home');

    await page.waitForFunction(
      ({ originalLines, modifiedLines }) => {
        const count = selector => new Set(
          [...document.querySelectorAll(selector)].map(e => Number(e.textContent)),
        ).size;
        return count('.original-in-monaco-diff-editor .line-numbers') >= originalLines
          && count('.modified-in-monaco-diff-editor .line-numbers') >= modifiedLines;
      },
      { originalLines: pair.originalLines, modifiedLines: pair.modifiedLines },
      { timeout: 60_000 },
    );

    const records = await page.evaluate(extractRecords, options);
    validateRecords(records, pair);
    await fs.writeFile(path.join(results, `${pair.id}.jsonl`), records);
    if (index + 1 < pairs.length) {
      await page.keyboard.press('Control+Alt+n');
    }
  }
} finally {
  await browser.close();
  server.dispose();
}

function validateRecords(text, pair) {
  const records = text.trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
  const lines = records.filter(record => record.type === 'line');
  const lineNumbers = side => new Set(lines.flatMap(line => line[side] === null ? [] : [line[side]]));
  const original = lineNumbers('original');
  const modified = lineNumbers('modified');
  if (original.size !== pair.originalLines || modified.size !== pair.modifiedLines) {
    throw new Error(
      `${options.layout} extractor missed lines for ${pair.id}: `
      + `original ${original.size}/${pair.originalLines}, modified ${modified.size}/${pair.modifiedLines}`,
    );
  }
  let previousOriginal = 0;
  let previousModified = 0;
  lines.forEach((line, index) => {
    if (line.index !== index) throw new Error(`${pair.id}: line indices are not contiguous`);
    if (line.original !== null) {
      if (line.original < previousOriginal) throw new Error(`${pair.id}: original lines are not ordered`);
      previousOriginal = line.original;
    }
    if (line.modified !== null) {
      if (line.modified < previousModified) throw new Error(`${pair.id}: modified lines are not ordered`);
      previousModified = line.modified;
    }
  });
}

function viewportWidth() {
  return options.wrap ? 800 : 1600;
}

async function viewportHeight(pair) {
  const lineHeight = 18;
  if (!options.wrap) {
    return Math.min(100000, (pair.originalLines + pair.modifiedLines) * lineHeight + 500);
  }
  const sizes = await Promise.all([pair.original, pair.modified].map(async file => (
    (await fs.stat(path.join(workspace, file))).size
  )));
  const estimatedLines = Math.max(
    pair.originalLines,
    pair.modifiedLines,
    Math.ceil((sizes[0] + sizes[1]) / options.wrap_column) * 3,
  );
  return Math.min(1000000, estimatedLines * lineHeight + 500);
}

async function freePort() {
  const server = net.createServer();
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  const { port } = server.address();
  await new Promise(resolve => server.close(resolve));
  return port;
}

function extractRecords(options) {
  function alignWrappedReplacementLines(records) {
    const lines = records.filter(record => record.type === 'line');
    for (let index = 0; index + 1 < lines.length; index++) {
      const first = lines[index];
      const second = lines[index + 1];
      if (first.original !== null && first.modified === null
        && second.original === null && second.modified !== null) {
        second.original = first.original;
        first.original = null;
      }
    }
  }

  function topOf(editor, node) {
    return Math.round(node.getBoundingClientRect().top - editor.getBoundingClientRect().top);
  }

  function extractLines(editor) {
    const lines = new Map();
    const highlights = new Map();
    const numbered = new Map();
    for (const element of editor.querySelectorAll('.margin-view-overlays > div')) {
      const number = element.querySelector('.line-numbers');
      if (number) numbered.set(topOf(editor, element), Number(number.textContent));
    }

    const physicalLines = [];
    for (const element of editor.querySelectorAll('.view-line')) {
      physicalLines.push({ top: topOf(editor, element), filler: false });
    }
    for (const zone of editor.querySelectorAll('.diagonal-fill')) {
      const lineHeight = Math.round(
        editor.querySelector('.view-line')?.getBoundingClientRect().height || 18,
      );
      const top = topOf(editor, zone);
      const count = Math.max(1, Math.round(zone.getBoundingClientRect().height / lineHeight));
      for (let index = 0; index < count; index++) {
        physicalLines.push({ top: top + index * lineHeight, filler: true });
      }
    }
    physicalLines.sort((a, b) => a.top - b.top);

    let previous = null;
    for (const physicalLine of physicalLines) {
      const number = numbered.get(physicalLine.top);
      if (number !== undefined) previous = number;
      else if (physicalLine.filler) previous = null;
      lines.set(physicalLine.top, number ?? null);
      highlights.set(physicalLine.top, number ?? (physicalLine.filler ? null : previous));
    }
    return { lines, highlights, numbered, physical: physicalLines };
  }

  function cellWidth(editor) {
    const source = editor.querySelector('.view-lines');
    const style = getComputedStyle(source);
    const sample = document.createElement('span');
    sample.textContent = '00000000000000000000';
    Object.assign(sample.style, {
      position: 'absolute',
      visibility: 'hidden',
      whiteSpace: 'pre',
      fontFamily: style.fontFamily,
      fontSize: style.fontSize,
      fontWeight: style.fontWeight,
      fontFeatureSettings: style.fontFeatureSettings,
      letterSpacing: style.letterSpacing,
    });
    editor.appendChild(sample);
    const width = sample.getBoundingClientRect().width / 20;
    sample.remove();
    return width;
  }

  function highlight(byLine, side, line) {
    let record = byLine.get(line);
    if (!record) {
      record = {
        type: 'highlight',
        side,
        line,
        line_background: null,
        gutter_background: null,
        characters: [],
        empty_markers: [],
      };
      byLine.set(line, record);
    }
    return record;
  }

  const original = document.querySelector('.original-in-monaco-diff-editor');
  const modified = document.querySelector('.modified-in-monaco-diff-editor');
  const originalLines = extractLines(original);
  const modifiedLines = extractLines(modified);
  const records = [];
  const tops = [...new Set([
    ...originalLines.lines.keys(),
    ...modifiedLines.lines.keys(),
  ])].sort((a, b) => a - b);
  tops.forEach((top, index) => {
    records.push({
      type: 'line',
      index,
      original: originalLines.lines.get(top) ?? null,
      modified: modifiedLines.lines.get(top) ?? null,
    });
  });
  if (options.wrap && options.layout === 'side-by-side') {
    alignWrappedReplacementLines(records);
  }

  for (const [side, editor, lineMap] of [
    ['original', original, originalLines],
    ['modified', modified, modifiedLines],
  ]) {
    const role = side === 'original' ? 'delete' : 'insert';
    const width = cellWidth(editor);
    const byLine = new Map();
    for (const element of editor.querySelectorAll('.view-overlays > div')) {
      const top = topOf(editor, element);
      const line = lineMap.highlights.get(top);
      if (line === undefined || line === null) continue;
      for (const decoration of element.querySelectorAll('.cdr')) {
        if (decoration.classList.contains(`line-${role}`)) {
          highlight(byLine, side, line).line_background = role;
          continue;
        }
        if (!decoration.classList.contains(`char-${role}`)) continue;
        const start = Math.round(parseFloat(decoration.style.left || '0') / width);
        if (decoration.classList.contains('diff-range-empty')) {
          highlight(byLine, side, line).empty_markers.push(start);
          continue;
        }
        if (decoration.style.width === '0px') continue;
        const fill = decoration.style.width === '100%';
        highlight(byLine, side, line).characters.push({
          start,
          end: fill ? null : start + Math.round(parseFloat(decoration.style.width) / width),
          fill_to_edge: fill,
        });
      }
    }
    for (const element of editor.querySelectorAll('.margin-view-overlays > div')) {
      const line = lineMap.highlights.get(topOf(editor, element));
      if (line !== undefined && line !== null && element.querySelector(`.gutter-${role}`)) {
        highlight(byLine, side, line).gutter_background = role;
      }
    }
    records.push(...[...byLine.values()].sort((a, b) => a.line - b.line));
  }
  return `${records.map(record => JSON.stringify(record)).join('\n')}\n`;
}
