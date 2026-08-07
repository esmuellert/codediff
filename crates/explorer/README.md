# explorer

The list of changed files, as a model: which section a file is in, how the
directories nest, what is folded, and which row is selected.

Pure. It is handed files and gives back rows. It cannot read a repository —
`cargo xtask lint-arch` forbids the edge — and it cannot draw, because a row
here is **facts**, never text and never a colour.

That split is the point. The plugin this replaces put building, filtering,
folding, formatting and drawing in one 674-line file, and every question about
one of them had to be asked of all five. Here the model can be checked with no
terminal, and the drawing can be checked with no repository.

## What a row is

Three facts: which node it shows, where it sits, and what it is.

```rust,ignore
Row {
    node: NodeId,
    guides: Some(Guides { ancestors: vec![false], is_last: true }),
    content: Content::File {
        name: "parser.rs".into(),
        moved_from: None,
        stats: Some(Stats { added: 12, removed: 3 }),
        change: ChangeType::Modified,
    },
}
```

Which `ui` draws as:

```text
│ └ parser.rs                        +12 -3  M
```

**Every character in that line is `ui`'s choice, not this crate's.** `└` rather
than `\`, `▾` rather than a nerd-font folder, `M` rather than `Modified`, and
what is dropped first when the pane is too narrow — all of it needs a terminal
to decide, and none of it can be decided here.

That is the division `align` already keeps: it reports that a view line is a
gap, and never that a gap is drawn `╱`. This crate used to hold both halves,
and the cost was that the one piece of it that was general — fitting a row into
a narrow pane — could not be reused by anything that was not a file list. It is
`ui::render::fit` now, where the status line can reach it: that one shortens a
path by the same rule, written a second time and by hand, and gets it wrong.
See [D65](../../docs/plan/05-decisions.md#d65) and
[B9](../../docs/plan/06-known-bugs.md).

`guides` says where a row sits rather than how deep it is: for each level
above, whether that ancestor was the last of its siblings. That is exactly the
question "does this column need a guide, or blank space" — a fact about the
walk, not a property of the node.
