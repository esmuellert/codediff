# AGENTS.md

Rules for making changes in this repository.

## Editing

**Use the built-in edit and view tools. Do not use Python, `sed` or shell redirection for edits unless there is no other way.**
The edit tool fails when the text is not there. A script does not — it changes nothing and reports success.

**Read the file and confirm the text is there before you edit it.**
An edit that silently does nothing is worse than one that fails, because everything after it is built on a false result.

## Writing

**Match the comment style around you. Keep inline comments short.**
A comment three times longer than its neighbours draws the eye to the wrong place.

**Use the name Neovim, VSCode or `codediff.nvim` already uses. Look it up before you invent one.**
These are the references this project follows. Most things already have a name — `:help group-name` had ours.

## Verifying

**Do not state a number or a cause unless you measured it.**
Say what you measured. A guess that sounds precise is worse than no number.

**Break the code on purpose and watch a new test fail. Only then trust it.**
A test that has never failed proves nothing. Breaking the code also finds design faults that reading it does not.

## Changing

**When you move code, only move it.**
Do not rename, reword or reshape it in the same step. A reader cannot tell a move from an edit when both are in one diff.
