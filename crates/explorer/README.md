# explorer

The list of changed files, as a model: which section a file is in, how the
directories nest, what is folded, and which row is selected.

Pure. It is handed files and gives back rows. It cannot read a repository —
`cargo xtask lint-arch` forbids the edge — and it cannot draw, because a row
here is text and a classification, never a colour or a cell.

That split is the point. The plugin this replaces put building, filtering,
folding, formatting and drawing in one 674-line file, and every question about
one of them had to be asked of all five. Here the model can be checked with no
terminal, and the drawing can be checked with no repository.

## What a row is

A row is left-hand regions and right-hand regions, with a gap between them:

```text
├  src/parser                        +12 -3  M
└──────────────┘                     └────────┘
     left                               right
```

When the pane is too narrow for both, regions are dropped in the order their
`priority` says, and the longest survivor is cut with an ellipsis. So the file
name is the last thing to go, and the status letter never moves.
