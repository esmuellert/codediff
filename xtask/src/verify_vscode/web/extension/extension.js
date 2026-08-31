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

async function open(index) {
  if (index >= pairs.length) return;
  current = index;
  const pair = pairs[current];
  status.text = `PARITY:${pair.id}`;
  status.show();
  await vscode.commands.executeCommand(
    'vscode.diff',
    vscode.Uri.joinPath(pair.root, pair.original),
    vscode.Uri.joinPath(pair.root, pair.modified),
    pair.id,
    { preview: true },
  );
}

async function activate(context) {
  status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 10_000);
  context.subscriptions.push(status);
  context.subscriptions.push(vscode.commands.registerCommand('codediffParity.next', () => open(current + 1)));
  const global = vscode.ConfigurationTarget.Global;
  await vscode.workspace.getConfiguration('diffEditor').update('renderSideBySide', true, global);
  await vscode.workspace.getConfiguration('diffEditor').update('useInlineViewWhenSpaceIsLimited', false, global);
  await vscode.workspace.getConfiguration('diffEditor').update('ignoreTrimWhitespace', true, global);
  await vscode.workspace.getConfiguration('diffEditor').update('experimental.showMoves', false, global);
  await vscode.workspace.getConfiguration('diffEditor').update('hideUnchangedRegions.enabled', false, global);
  await vscode.workspace.getConfiguration('editor').update('wordWrap', 'off', global);
  await vscode.workspace.getConfiguration('editor').update('minimap.enabled', false, global);
  await readPairs();
  await open(0);
}

module.exports = { activate };
