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
const manifestPath = require.resolve('@codediff/vscode-extension/package.json');
const extension = path.dirname(manifestPath);
const manifest = JSON.parse(await fs.readFile(manifestPath, 'utf8'));
const commit = manifest.vscodeCommit;

const pairs = (await fs.readFile(path.join(workspace, 'pairs.txt'), 'utf8'))
  .trim()
  .split('\n')
  .filter(Boolean)
  .map(line => {
    const [id, , , originalLines, modifiedLines] = line.split('\t');
    return { id, originalLines: Number(originalLines), modifiedLines: Number(modifiedLines) };
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
const height = Math.min(100000, Math.max(...pairs.map(p => Math.max(p.originalLines, p.modifiedLines))) * 18 + 500);
const page = await browser.newPage({ viewport: { width: 1600, height } });

try {
  await page.goto(`http://localhost:${port}`);
  await fs.mkdir(results, { recursive: true });
  for (let index = 0; index < pairs.length; index++) {
    const pair = pairs[index];
    await page.getByText(`PARITY:${pair.id}`, { exact: true }).waitFor({ timeout: 60_000 });
    await page.locator('.monaco-diff-editor.side-by-side').waitFor({ timeout: 60_000 });
    const modified = page.locator('.modified-in-monaco-diff-editor');
    await modified.click({ position: { x: 100, y: 40 } });
    await page.keyboard.press(process.platform === 'darwin' ? 'Meta+ArrowUp' : 'Control+Home');

    await page.waitForFunction(
      ({ originalLines, modifiedLines }) => {
        const count = selector => new Set(
          [...document.querySelectorAll(selector)].map(e => Number(e.textContent))
        ).size;
        return count('.original-in-monaco-diff-editor .line-numbers') >= originalLines
          && count('.modified-in-monaco-diff-editor .line-numbers') >= modifiedLines;
      },
      { originalLines: pair.originalLines, modifiedLines: pair.modifiedLines },
      { timeout: 60_000 },
    );

    const records = await page.evaluate(extract);
    await fs.writeFile(path.join(results, `${pair.id}.jsonl`), records);
    if (index + 1 < pairs.length) {
      await page.keyboard.press('Control+Alt+n');
    }
  }
} finally {
  await browser.close();
  server.dispose();
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

function extract() {
  function lines(editor) {
    const result = new Map();
    for (const row of editor.querySelectorAll('.margin-view-overlays > div')) {
      const number = row.querySelector('.line-numbers');
      if (number) result.set(Math.round(parseFloat(row.style.top)), Number(number.textContent));
    }
    return result;
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
  const originalLines = lines(original);
  const modifiedLines = lines(modified);
  const records = [];
  const tops = [...new Set([...originalLines.keys(), ...modifiedLines.keys()])].sort((a, b) => a - b);
  tops.forEach((top, index) => {
    records.push({
      type: 'row',
      index,
      original: originalLines.get(top) ?? null,
      modified: modifiedLines.get(top) ?? null,
    });
  });

  for (const [side, editor, lineMap] of [
    ['original', original, originalLines],
    ['modified', modified, modifiedLines],
  ]) {
    const role = side === 'original' ? 'delete' : 'insert';
    const width = cellWidth(editor);
    const byLine = new Map();
    for (const row of editor.querySelectorAll('.view-overlays > div')) {
      const line = lineMap.get(Math.round(parseFloat(row.style.top)));
      if (!line) continue;
      for (const decoration of row.querySelectorAll('.cdr')) {
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
    for (const row of editor.querySelectorAll('.margin-view-overlays > div')) {
      const line = lineMap.get(Math.round(parseFloat(row.style.top)));
      if (line && row.querySelector(`.gutter-${role}`)) {
        highlight(byLine, side, line).gutter_background = role;
      }
    }
    records.push(...[...byLine.values()].sort((a, b) => a.line - b.line));
  }
  return `${records.map(record => JSON.stringify(record)).join('\n')}\n`;
}
