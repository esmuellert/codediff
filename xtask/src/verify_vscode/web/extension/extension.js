const vscode = require('vscode');

let pairs = [];
let current = -1;
let status;

async function readPairs() {
  const root = vscode.workspace.workspaceFolders?.[0]?.uri;
  if (!root) throw new Error('codediff parity workspace is missing');
  const bytes = await vscode.workspace.fs.readFile(vscode.Uri.joinPath(root, 'pairs.txt'));
  pairs = new TextDecoder().decode(bytes).trim().split('\n').filter(Boolean).map(line => {
    const [id, original, modified] = line.split('\t');
    return { id, original, modified, root };
  });
}

async function readOptions() {
  const root = vscode.workspace.workspaceFolders?.[0]?.uri;
  if (!root) throw new Error('codediff parity workspace is missing');
  const bytes = await vscode.workspace.fs.readFile(vscode.Uri.joinPath(root, 'options.json'));
  const options = JSON.parse(new TextDecoder().decode(bytes));
  if (typeof options.ignore_trim_whitespace !== 'boolean') {
    throw new Error('ignore_trim_whitespace must be boolean');
  }
  if (typeof options.wrap !== 'boolean') {
    throw new Error('wrap must be boolean');
  }
  if (!Number.isInteger(options.wrap_column) || options.wrap_column < 1) {
    throw new Error('wrap_column must be a positive integer');
  }
  if (!['side-by-side', 'inline'].includes(options.layout)) {
    throw new Error('layout must be side-by-side or inline');
  }
  return options;
}

async function open(index) {
  if (index >= pairs.length) return;
  current = index;
  const pair = pairs[current];
  await vscode.commands.executeCommand(
    'vscode.diff',
    vscode.Uri.joinPath(pair.root, pair.original),
    vscode.Uri.joinPath(pair.root, pair.modified),
    pair.id,
    { preview: true },
  );
  status.text = `PARITY:${pair.id}`;
  status.show();
}

async function activate(context) {
  status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 10_000);
  context.subscriptions.push(status);
  context.subscriptions.push(vscode.commands.registerCommand('codediffParity.next', () => open(current + 1)));
  const options = await readOptions();
  const target = vscode.ConfigurationTarget.Workspace;
  const sideBySide = options.layout === 'side-by-side';
  await vscode.workspace.getConfiguration('diffEditor').update('renderSideBySide', sideBySide, target);
  await vscode.workspace.getConfiguration('diffEditor').update('useInlineViewWhenSpaceIsLimited', false, target);
  await vscode.workspace.getConfiguration('diffEditor').update('ignoreTrimWhitespace', options.ignore_trim_whitespace, target);
  await vscode.workspace.getConfiguration('diffEditor').update('maxComputationTime', 0, target);
  await vscode.workspace.getConfiguration('diffEditor').update('experimental.showMoves', false, target);
  await vscode.workspace.getConfiguration('diffEditor').update('experimental.useTrueInlineView', false, target);
  await vscode.workspace.getConfiguration('diffEditor').update('hideUnchangedRegions.enabled', false, target);
  await vscode.workspace.getConfiguration('diffEditor').update(
    'wordWrap',
    options.wrap ? 'inherit' : 'off',
    target,
  );
  await vscode.workspace.getConfiguration('editor').update(
    'wordWrap',
    options.wrap ? 'wordWrapColumn' : 'off',
    target,
  );
  await vscode.workspace.getConfiguration('editor').update('wordWrapColumn', options.wrap_column, target);
  await vscode.workspace.getConfiguration('editor').update('colorDecorators', false, target);
  await vscode.workspace.getConfiguration('editor').update('minimap.enabled', false, target);
  await readPairs();
  await open(0);
}

module.exports = { activate };
