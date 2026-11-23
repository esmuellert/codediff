//! Pinned `Working tree` row for [`super::RevisionPicker`].

use std::rc::Rc;

use loom::{
    Basis, Gap, GapProps, Layout, Node as LoomNode, Row, RowProps, Scope, Text, TextProps,
    component, rsx, use_context,
};

use crate::components::context::Ui;

const LABEL: &str = "Working tree";

/// Left margin plus graph-column spacer before `"Working tree"`, matching commit rows.
pub(super) fn worktree_indent(area_width: u16, lane_count: usize, filtering: bool) -> u16 {
    if filtering || lane_count == 0 {
        return 1;
    }
    let max_graph_width = if area_width > 24 {
        area_width.saturating_sub(20).min(area_width / 2)
    } else {
        area_width.saturating_sub(1)
    };
    let graph_width = (lane_count as u16).saturating_mul(2).min(max_graph_width);
    1u16.saturating_add(graph_width).saturating_add(1)
}

#[component]
pub(super) fn WorktreeRow(
    scope: &mut Scope,
    summary: Rc<str>,
    indent: u16,
    selected: bool,
) -> LoomNode {
    let theme = use_context::<Ui>(scope).theme;
    let selected = *selected;
    let indent = *indent;
    let base = if selected {
        theme.normal.patch(theme.cursor_line)
    } else {
        theme.normal
    };

    let mut children = Vec::with_capacity(4);
    if indent > 0 {
        children.push(rsx! {
            Gap {
                key: 0u32,
                layout: Layout {
                    basis: Basis::Length(indent),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
        });
    }

    children.push(rsx! {
        Text {
            key: 1u32,
            text: Rc::from(LABEL),
            style: base.fg(theme.tree.name),
            layout: Layout {
                basis: Basis::Length(LABEL.len() as u16),
                shrink: 1,
                clip: true,
                ..Default::default()
            },
            ..
        }
    });

    if !summary.is_empty() {
        children.push(rsx! {
            Gap {
                key: 2u32,
                layout: Layout {
                    basis: Basis::Length(2),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
        });
        children.push(rsx! {
            Text {
                key: 3u32,
                text: Rc::clone(summary),
                style: base.fg(theme.tree.previous),
                layout: Layout {
                    grow: 1,
                    shrink: 1,
                    clip: true,
                    ..Default::default()
                },
                ..
            }
        });
    }

    rsx! {
        Row {
            layout: Layout {
                basis: Basis::Length(1),
                shrink: 0,
                clip: true,
                fill: Some(base),
                ..Default::default()
            },
            ..,
            { children }
        }
    }
}
