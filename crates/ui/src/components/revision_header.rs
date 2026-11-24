//! One-line revision header for selecting left/right endpoints and swapping them.

use std::path::Path;
use std::rc::Rc;

use crate::RevisionRange;
use loom::{
    Basis, Bubble, Gap, GapProps, Layout, Listeners, Node, Row, RowProps, Scope, Text, TextProps,
    component, crossterm::event::MouseButton, rsx, use_context, use_effect, use_measure, use_memo,
    use_state,
};
use ratatui::style::{Color, Modifier, Style};

use super::context::Ui;
use super::revisions::{RevisionSide, find_commit, head_commit, range_revs, swap_revision_range};
use crate::Commit;
use crate::services::{Resource, VcsService, WatcherService};
use crate::theme::Theme;

const ICON_BRANCH: &str = "\u{f418}";
const ICON_TAG: &str = "\u{f412}";
const ICON_COMMIT: &str = "\u{f417}";
const ICON_WORKTREE: &str = "\u{e5fe}";
const ICON_INDEX: &str = "\u{f046}";

#[derive(Clone, Copy)]
enum EndpointGlyph {
    Branch,
    Tag,
    Commit,
    Worktree,
    Index,
}

impl EndpointGlyph {
    fn glyph(self) -> &'static str {
        match self {
            Self::Branch => ICON_BRANCH,
            Self::Tag => ICON_TAG,
            Self::Commit => ICON_COMMIT,
            Self::Worktree => ICON_WORKTREE,
            Self::Index => ICON_INDEX,
        }
    }

    fn colour(self, theme: &Theme) -> Color {
        match self {
            Self::Branch => theme.tree.directory,
            Self::Tag | Self::Worktree => theme.change.modified,
            Self::Commit | Self::Index => theme.tree.previous,
        }
    }
}

struct EndpointLabel {
    glyph: EndpointGlyph,
    text: String,
}

fn cell_width(text: &str) -> u16 {
    line_index::LineIndex::new(text, 1).width().0 as u16
}

fn short_hash(hash: &str) -> &str {
    &hash[..hash.len().min(7)]
}

fn format_commit(commit: &Commit) -> EndpointLabel {
    if let Some(branch) = commit.branches.first() {
        return EndpointLabel {
            glyph: EndpointGlyph::Branch,
            text: branch.clone(),
        };
    }
    if let Some(tag) = commit.tags.first() {
        return EndpointLabel {
            glyph: EndpointGlyph::Tag,
            text: tag.clone(),
        };
    }
    if let Some(remote) = commit.remotes.first() {
        return EndpointLabel {
            glyph: EndpointGlyph::Branch,
            text: remote.clone(),
        };
    }
    EndpointLabel {
        glyph: EndpointGlyph::Commit,
        text: short_hash(commit.hash.as_str()).to_owned(),
    }
}

fn format_head(commits: &[Commit]) -> EndpointLabel {
    head_commit(commits)
        .map(format_commit)
        .unwrap_or_else(|| EndpointLabel {
            glyph: EndpointGlyph::Commit,
            text: "HEAD".to_owned(),
        })
}

fn format_rev(rev: &str, commits: &[Commit]) -> EndpointLabel {
    if rev == "HEAD" {
        return format_head(commits);
    }
    if let Some(commit) = find_commit(rev, commits) {
        if commit.branches.iter().any(|b| b == rev) || commit.remotes.iter().any(|r| r == rev) {
            return EndpointLabel {
                glyph: EndpointGlyph::Branch,
                text: rev.to_owned(),
            };
        }
        if commit.tags.iter().any(|t| t == rev) {
            return EndpointLabel {
                glyph: EndpointGlyph::Tag,
                text: rev.to_owned(),
            };
        }
        return format_commit(commit);
    }
    if rev.len() >= 12 && rev.bytes().all(|b| b.is_ascii_hexdigit()) {
        return EndpointLabel {
            glyph: EndpointGlyph::Commit,
            text: short_hash(rev).to_owned(),
        };
    }
    EndpointLabel {
        glyph: EndpointGlyph::Commit,
        text: rev.to_owned(),
    }
}

fn current_branch(commits: &[Commit]) -> Option<String> {
    let head = head_commit(commits)?;
    if let Some(branch) = head.branches.first() {
        return Some(branch.clone());
    }
    Some(short_hash(head.hash.as_str()).to_owned())
}

fn pill_base_style(theme: &Theme, active: bool) -> Style {
    let mut style = theme.normal.patch(theme.cursor_line);
    if active {
        style = style.add_modifier(Modifier::UNDERLINED);
        if let Some(colour) = theme.border_focused.fg {
            style = style.underline_color(colour);
        }
    }
    style
}

fn endpoint_pill(
    key: u32,
    endpoint: &EndpointLabel,
    active: bool,
    theme: &Theme,
    on_pick: impl Fn() + 'static,
) -> (Node, u16) {
    let icon_text = format!(" {} ", endpoint.glyph.glyph());
    let chevron_text = " ▾ ";

    let icon_width = cell_width(&icon_text);
    let label_width = cell_width(&endpoint.text);
    let chevron_width = cell_width(chevron_text);
    let total_width = icon_width
        .saturating_add(label_width)
        .saturating_add(chevron_width);

    let base_style = pill_base_style(theme, active);
    let accent = theme.border_focused.fg.unwrap_or(theme.tree.name);
    let icon_style = if active {
        base_style.fg(accent)
    } else {
        base_style.fg(endpoint.glyph.colour(theme))
    };
    let label_style = if active {
        base_style.fg(accent)
    } else {
        base_style.fg(theme.tree.name)
    };
    let chevron_style = if active {
        base_style.fg(accent)
    } else {
        base_style.fg(theme.tree.previous)
    };

    let listeners = Listeners::new().on_mouse_down(move |mouse| {
        if mouse.button == Some(MouseButton::Left) {
            on_pick();
            Bubble::Stop
        } else {
            Bubble::Continue
        }
    });

    let node = rsx! {
        Row {
            key,
            listeners,
            layout: Layout {
                basis: Basis::Length(total_width),
                shrink: 0,
                fill: Some(base_style),
                ..Default::default()
            },
            ..,
            Text {
                key: 0u32,
                text: Rc::from(icon_text),
                style: icon_style,
                layout: Layout {
                    basis: Basis::Length(icon_width),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
            Text {
                key: 1u32,
                text: Rc::from(endpoint.text.as_str()),
                style: label_style,
                layout: Layout {
                    basis: Basis::Length(label_width),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
            Text {
                key: 2u32,
                text: Rc::from(chevron_text),
                style: chevron_style,
                layout: Layout {
                    basis: Basis::Length(chevron_width),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
        }
    };

    (node, total_width)
}

fn swap_part(show: bool, width: u16, theme: &Theme, on_swap: impl Fn() + 'static) -> Node {
    if !show {
        return Node::Empty;
    }
    let text = "  ⇄ ";
    let total_width = 1u16.saturating_add(width);
    let style = theme.normal.fg(theme.tree.previous);
    let listeners = Listeners::new().on_mouse_down(move |mouse| {
        if mouse.button == Some(MouseButton::Left) {
            on_swap();
            Bubble::Stop
        } else {
            Bubble::Continue
        }
    });
    rsx! {
        Row {
            key: 4u32,
            listeners,
            layout: Layout {
                basis: Basis::Length(total_width),
                shrink: 0,
                fill: Some(theme.normal),
                ..Default::default()
            },
            ..,
            Text {
                key: 0u32,
                text: Rc::from(text),
                style,
                layout: Layout {
                    basis: Basis::Length(total_width),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
        }
    }
}

fn meta_part(
    show: bool,
    repo: Option<String>,
    repo_width: u16,
    branch_text: Option<String>,
    branch_width: u16,
    theme: &Theme,
) -> Node {
    if !show {
        return Node::Empty;
    }
    let repo_node = match repo {
        Some(repo_name) => rsx! {
            Text {
                key: 0u32,
                text: Rc::from(repo_name),
                style: theme.normal.fg(theme.tree.previous),
                layout: Layout {
                    basis: Basis::Length(repo_width),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
        },
        None => Node::Empty,
    };
    let branch_node = match branch_text {
        Some(branch_label) => rsx! {
            Text {
                key: 1u32,
                text: Rc::from(branch_label),
                style: theme.normal.fg(theme.tree.directory),
                layout: Layout {
                    basis: Basis::Length(branch_width),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
        },
        None => Node::Empty,
    };
    rsx! {
        Row {
            key: 6u32,
            layout: Layout {
                basis: Basis::Length(repo_width.saturating_add(branch_width)),
                shrink: 0,
                ..Default::default()
            },
            ..,
            { repo_node }
            { branch_node }
        }
    }
}

fn use_range_commits(
    scope: &mut Scope,
    repo: &Rc<Path>,
    revision_range: &RevisionRange,
    vcs: Option<&Rc<VcsService>>,
    watcher: Option<&Rc<WatcherService>>,
) -> Rc<Vec<Commit>> {
    let (commits, set_commits) = use_state(scope, || Rc::new(Vec::<Commit>::new()));
    let (refreshes, set_refreshes) = use_state(scope, || 0u32);

    let watcher_for_effect = watcher.map(Rc::clone);
    use_effect(scope, Rc::clone(repo), move || {
        let Some(watcher) = watcher_for_effect else {
            return;
        };
        watcher.subscribe(move |refresh: watcher::Refresh| {
            if refresh.head || refresh.refs {
                set_refreshes.update(|n| *n += 1);
            }
        });
    });

    let revs = range_revs(revision_range);
    let revs_for_memo = revs.clone();
    let repo_for_commits = Rc::clone(repo);
    let vcs_for_commits = vcs.map(Rc::clone);
    let loaded_commits = use_memo(
        scope,
        (Rc::clone(repo), revs.clone(), refreshes),
        move || vcs_for_commits.map(|vcs| vcs.resolve_commits(&repo_for_commits, revs_for_memo)),
    );

    let mut effective_commits = Rc::clone(&commits);
    if let Some(Ok(resolved)) = loaded_commits
        .as_ref()
        .as_ref()
        .and_then(|res| res.is_ready().then(|| res.take()).flatten())
        && *effective_commits != resolved
    {
        let next = Rc::new(resolved);
        effective_commits = Rc::clone(&next);
        set_commits.set(next);
    }

    let commits_ready = loaded_commits
        .as_ref()
        .as_ref()
        .is_some_and(Resource::is_ready);
    let commits_for_effect = Rc::clone(&loaded_commits);
    use_effect(
        scope,
        (Rc::clone(repo), revs, refreshes, commits_ready),
        move || {
            let Some(Ok(resolved)) = commits_for_effect
                .as_ref()
                .as_ref()
                .and_then(Resource::take)
            else {
                return;
            };
            set_commits.update(|current| {
                if **current != resolved {
                    *current = Rc::new(resolved);
                }
            });
        },
    );

    effective_commits
}

#[component]
pub fn RevisionHeader(scope: &mut Scope) -> Node {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let (node_ref, size) = use_measure(scope);

    let commits = use_range_commits(
        scope,
        &ctx.repo,
        &ctx.revision_range,
        ctx.vcs.as_ref(),
        ctx.watcher.as_ref(),
    );
    let commits = &commits;
    let (left, right, merge_base, can_swap) = match &ctx.revision_range {
        RevisionRange::Worktree => (
            format_head(commits),
            EndpointLabel {
                glyph: EndpointGlyph::Worktree,
                text: "Working tree".to_owned(),
            },
            false,
            false,
        ),
        RevisionRange::Staged(rev) => (
            format_rev(rev, commits),
            EndpointLabel {
                glyph: EndpointGlyph::Index,
                text: "Index".to_owned(),
            },
            false,
            false,
        ),
        RevisionRange::Against(rev) => (
            format_rev(rev, commits),
            EndpointLabel {
                glyph: EndpointGlyph::Worktree,
                text: "Working tree".to_owned(),
            },
            false,
            false,
        ),
        RevisionRange::Between(from, to) => (
            format_rev(from, commits),
            format_rev(to, commits),
            false,
            true,
        ),
        RevisionRange::MergeBase(base, head) => (
            format_rev(base, commits),
            format_rev(head, commits),
            true,
            true,
        ),
    };

    let from_text = " From ";
    let from_width = cell_width(from_text);
    let connector_text = if merge_base { " ... " } else { "  to  " };
    let connector_width = cell_width(connector_text);

    let set_picking_left = ctx.set_picking;
    let (left_pill, left_width) = endpoint_pill(
        1,
        &left,
        ctx.picking == Some(RevisionSide::Left),
        theme,
        move || {
            if let Some(set_picking) = set_picking_left {
                set_picking.update(|side| {
                    *side = (*side != Some(RevisionSide::Left)).then_some(RevisionSide::Left);
                });
            }
        },
    );

    let set_picking_right = ctx.set_picking;
    let (right_pill, right_width) = endpoint_pill(
        3,
        &right,
        ctx.picking == Some(RevisionSide::Right),
        theme,
        move || {
            if let Some(set_picking) = set_picking_right {
                set_picking.update(|side| {
                    *side = (*side != Some(RevisionSide::Right)).then_some(RevisionSide::Right);
                });
            }
        },
    );

    let swap_width = cell_width(" ⇄ ");
    let repo = ctx
        .repo
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_owned);
    let branch = current_branch(commits);

    let repo_width = repo.as_deref().map_or(0, cell_width);
    let branch_text = branch.as_deref().map(|b| {
        if repo.is_some() {
            format!("  {ICON_BRANCH} {b}")
        } else {
            format!("{ICON_BRANCH} {b}")
        }
    });
    let branch_width = branch_text.as_deref().map_or(0, cell_width);
    let meta_width = repo_width.saturating_add(branch_width);

    let core_width = from_width
        .saturating_add(left_width)
        .saturating_add(connector_width)
        .saturating_add(right_width)
        .saturating_add(1);

    let available = size.width;
    let show_swap = can_swap
        && (available == 0 || core_width.saturating_add(1).saturating_add(swap_width) <= available);
    let used_left = core_width.saturating_add(if show_swap {
        1u16.saturating_add(swap_width)
    } else {
        0
    });
    let show_meta = meta_width > 0
        && (available == 0 || used_left.saturating_add(2).saturating_add(meta_width) <= available);

    let set_range_for_swap = ctx.set_revision_range;
    let range_for_swap = ctx.revision_range.clone();
    let swap = swap_part(show_swap, swap_width, theme, move || {
        if let Some(swapped) = swap_revision_range(&range_for_swap)
            && let Some(set_range) = set_range_for_swap
        {
            set_range.set(swapped);
        }
    });
    let meta = meta_part(
        show_meta,
        repo,
        repo_width,
        branch_text,
        branch_width,
        theme,
    );

    rsx! {
        Row {
            node_ref: Some(node_ref),
            layout: Layout {
                basis: Basis::Length(1),
                shrink: 0,
                clip: true,
                fill: Some(theme.normal),
                ..Default::default()
            },
            ..,
            Text {
                key: 0u32,
                text: Rc::from(from_text),
                style: theme.normal.fg(theme.tree.previous),
                layout: Layout {
                    basis: Basis::Length(from_width),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
            { left_pill }
            Text {
                key: 2u32,
                text: Rc::from(connector_text),
                style: theme.normal.fg(theme.tree.previous),
                layout: Layout {
                    basis: Basis::Length(connector_width),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
            { right_pill }
            { swap }
            Gap {
                key: 5u32,
                layout: Layout {
                    grow: 1,
                    shrink: 1,
                    ..Default::default()
                },
                ..
            }
            { meta }
            Gap {
                key: 7u32,
                layout: Layout {
                    basis: Basis::Length(1),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
        }
    }
}
