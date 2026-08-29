//! Turning a list of files into a flat list of nodes.

use std::collections::HashSet;

use file_types::File;

/// What one line of the explorer is. Built by `tree()`, read by Explorer.
#[derive(Clone)]
pub enum Node {
    Heading {
        name: &'static str,
        count: usize,
        added: u32,
        removed: u32,
    },
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

/// Groups files by revision pair, builds a tree per group, and prepends
/// a heading node for each.
pub fn grouped_tree(files: &[File], folded: &HashSet<String>) -> Vec<Node> {
    let groups = group(files);
    let mut out = Vec::new();
    for (heading, members) in &groups {
        let group_files: Vec<&File> = members.iter().map(|&i| &files[i]).collect();
        out.push(heading_node(heading, &group_files));

        let mut root: Vec<Item<'_>> = Vec::new();
        for &file in &group_files {
            let path = file.path().as_str();
            let (dirs, name) = split_path(path);
            insert(&mut root, &dirs, name, file);
        }
        flatten(&mut root);
        sort(&mut root);
        walk(&root, &[], "", folded, &mut out);
    }
    out
}

/// Groups files by revision pair, lists each file by its full path.
pub fn grouped_list(files: &[File]) -> Vec<Node> {
    let groups = group(files);
    let mut out = Vec::new();
    for (heading, members) in &groups {
        let group_files: Vec<&File> = members.iter().map(|&i| &files[i]).collect();
        out.push(heading_node(heading, &group_files));

        let mut sorted = group_files;
        sorted.sort_by(|a, b| a.path().as_str().cmp(b.path().as_str()));
        for file in sorted {
            out.push(Node::File {
                indent: String::new(),
                name: file.path().as_str().to_string(),
                file: file.clone(),
            });
        }
    }
    out
}

fn heading_node(name: &'static str, files: &[&File]) -> Node {
    let mut added = 0u32;
    let mut removed = 0u32;
    for f in files {
        if let Some(stats) = f.get_stats().filter(|s| !s.is_empty()) {
            added += stats.added;
            removed += stats.removed;
        }
    }
    Node::Heading { name, count: files.len(), added, removed }
}

/// Groups files by revision pair, preserving the order the first file of
/// each group arrived in.
fn group(files: &[File]) -> Vec<(&'static str, Vec<usize>)> {
    let mut groups: Vec<(file_types::Revs, Vec<usize>)> = Vec::new();
    for (index, file) in files.iter().enumerate() {
        let revs = file.revs();
        match groups.iter_mut().find(|(seen, _)| *seen == revs) {
            Some((_, members)) => members.push(index),
            None => groups.push((revs, vec![index])),
        }
    }
    groups
        .into_iter()
        .map(|(revs, members)| (revs.heading(), members))
        .collect()
}
