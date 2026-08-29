//! Turning a list of files into a flat list of nodes.

use std::collections::HashSet;

use file_types::File;

/// What one line of the explorer is. Built by `tree()`, read by Explorer.
#[derive(Clone)]
pub enum Node {
    Directory {
        indent: String,
        name: String,
        path: String,
        open: bool,
    },
    File {
        indent: String,
        name: String,
        file: File,
    },
}

pub fn tree(files: &[File], folded: &HashSet<String>) -> Vec<Node> {
    let mut root: Vec<Item<'_>> = Vec::new();
    for file in files {
        let path = file.path().as_str();
        let (dirs, name) = split_path(path);
        insert(&mut root, &dirs, name, file);
    }

    flatten(&mut root);
    sort(&mut root);

    let mut out = Vec::new();
    walk(&root, &[], "", folded, &mut out);
    out
}

enum Item<'a> {
    File { name: &'a str, file: &'a File },
    Directory { name: String, children: Vec<Item<'a>> },
}

impl<'a> Item<'a> {
    fn name(&self) -> &str {
        match self {
            Item::File { name, .. } => name,
            Item::Directory { name, .. } => name,
        }
    }

    fn is_directory(&self) -> bool {
        matches!(self, Item::Directory { .. })
    }
}

fn insert<'a>(items: &mut Vec<Item<'a>>, dirs: &[&str], name: &'a str, file: &'a File) {
    if dirs.is_empty() {
        items.push(Item::File { name, file });
        return;
    }

    let dir_name = dirs[0];
    let rest = &dirs[1..];

    let pos = items.iter().position(|item| {
        matches!(item, Item::Directory { name, .. } if name == dir_name)
    });

    match pos {
        Some(pos) => {
            let Item::Directory { children, .. } = &mut items[pos] else {
                unreachable!()
            };
            insert(children, rest, name, file);
        }
        None => {
            let mut children = Vec::new();
            insert(&mut children, rest, name, file);
            items.push(Item::Directory {
                name: dir_name.to_owned(),
                children,
            });
        }
    }
}

/// Collapses single-child directory chains: `src` → `view` → file becomes
/// `src/view` → file.
fn flatten(items: &mut Vec<Item<'_>>) {
    for item in items.iter_mut() {
        let Item::Directory { name, children } = item else { continue };
        flatten(children);
        if children.len() == 1 && children[0].is_directory() {
            let Item::Directory {
                name: child_name,
                children: grandchildren,
            } = children.remove(0)
            else {
                unreachable!()
            };
            *name = format!("{name}/{child_name}");
            *children = grandchildren;
        }
    }
}

fn sort(items: &mut Vec<Item<'_>>) {
    items.sort_by(|a, b| {
        b.is_directory()
            .cmp(&a.is_directory())
            .then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
    });
    for item in items.iter_mut() {
        if let Item::Directory { children, .. } = item {
            sort(children);
        }
    }
}

/// Depth-first walk, producing the flat output. `ancestors[i]` is true when
/// the ancestor at depth `i` was the last of its siblings.
fn walk(
    items: &[Item<'_>],
    ancestors: &[bool],
    path_prefix: &str,
    folded: &HashSet<String>,
    out: &mut Vec<Node>,
) {
    let last_index = items.len().saturating_sub(1);

    for (i, item) in items.iter().enumerate() {
        let is_last = i == last_index;
        let indent = markers(ancestors, is_last);

        match item {
            Item::Directory { name, children } => {
                let full_path = if path_prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{path_prefix}/{name}")
                };
                let open = !folded.contains(&full_path);

                out.push(Node::Directory {
                    indent,
                    name: name.clone(),
                    path: full_path.clone(),
                    open,
                });

                if open {
                    let mut next = ancestors.to_vec();
                    next.push(is_last);
                    walk(children, &next, &full_path, folded, out);
                }
            }
            Item::File { name, file } => {
                out.push(Node::File {
                    indent,
                    name: name.to_string(),
                    file: (*file).clone(),
                });
            }
        }
    }
}

fn markers(ancestors: &[bool], is_last: bool) -> String {
    let mut out = String::with_capacity(ancestors.len() * 4 + 4);
    for &was_last in ancestors {
        out.push_str(if was_last { "  " } else { "│ " });
    }
    out.push_str(if is_last { "└ " } else { "├ " });
    out
}

fn split_path(path: &str) -> (Vec<&str>, &str) {
    let mut segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let name = segments.pop().unwrap_or(path);
    (segments, name)
}
