//! How a key sequence is looked up.
//!
//! The bindings themselves live with the level that executes them — one file
//! each, `buffer`, `pane`, `tab`, `view`, `program` — so adding a command
//! means touching one file, not two.
//!
//! **Lookup walks the containment hierarchy, innermost first.** That single
//! fact does two jobs. It puts each level's bindings where the level is, and
//! it makes *shadowing* the answer to scoping: a buffer kind that binds `<`
//! claims it, and everywhere else the same key falls through to the tab. Exactly
//! how Neovim's buffer-local mappings shadow global ones — and the reason a
//! key's list and a key's executor need not be tied together.
//!
//! The tables are **data** — `const`, comparable, printable. A binding is a
//! sequence of keys and an [`Action`] *value*, never a closure. A closure
//! could not be rendered into a help screen, compared in a test, or held
//! without capturing references to everything it might touch.
//!
//! Lookup gives the flat lists in [`bindings`] **trie semantics**: an action lives only
//! at a leaf, so no binding may be a proper prefix of another. That is what
//! vim's own built-in keymap does — `g`, `d`, `z`, `[` and `]` are all
//! unbound alone — and it is why the resolver needs no clock. Ambiguity has
//! no good resolution: firing immediately makes the longer binding
//! unreachable, and waiting makes the shorter one feel broken. Vim needs
//! `timeoutlen` only because user mappings *may* create it.

use crokey::KeyCombination;

use crate::input::command::Action;
use align::DiffLayout;

use crate::input::{buffer, pane, program, tab, view};

/// Which set of bindings is live.
///
/// Decided by the focused buffer, and passed in rather than read from
/// anywhere, so this module depends on nothing — which is what lets the keymap
/// be built before the thing that decides focus exists.
///
/// A diff carries its [`DiffLayout`] rather than restating `SideBySide` and
/// `Inline`, because those are the same two things `align` already names and a
/// second definition is one that can drift. What this enum adds is the
/// buffers that are *not* diffs — which is exactly why it cannot simply be an
/// `Option<DiffLayout>`: the explorer is a third answer, not an absent one.
/// See D33.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeymapType {
    /// Two versions, laid out one of the two ways.
    Diff(DiffLayout),
    /// One version of a file, with nothing to compare it against.
    #[default]
    SingleFile,
    /// The list of changed files.
    Explorer,
}

impl KeymapType {
    pub const ALL: &'static [KeymapType] = &[
        KeymapType::Diff(DiffLayout::SideBySide),
        KeymapType::Diff(DiffLayout::Inline),
        KeymapType::SingleFile,
        KeymapType::Explorer,
    ];
}

/// One key sequence and what it does.
#[derive(Debug, Clone, Copy)]
pub struct Binding {
    /// One entry for a single key, more for a sequence such as `gg`.
    ///
    /// Written in **normalised** form — `key!(shift-g)` rather than
    /// `key!(G)` — because incoming events are normalised before lookup and
    /// the two must meet. A test checks every entry against its own
    /// normalisation.
    pub keys: &'static [KeyCombination],
    pub action: Action,
}

/// The outcome of looking a key sequence up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Match {
    /// This sequence is bound.
    Exact(Action),
    /// A proper prefix of at least one binding: more keys are needed.
    Prefix,
    /// Nothing starts with this.
    None,
}

/// Looks `keys` up, walking outwards from the focused buffer.
///
/// A linear scan of a few short lists. A real trie would be faster and is not
/// worth it: this runs once per keypress, not once per cell.
pub fn lookup(keymap_type: KeymapType, keys: &[KeyCombination]) -> Match {
    for list in live(keymap_type) {
        if let Some(binding) = list.iter().find(|b| b.keys == keys) {
            return Match::Exact(binding.action);
        }
    }
    for list in live(keymap_type) {
        if list.iter().any(|b| starts_with(b.keys, keys)) {
            return Match::Prefix;
        }
    }
    Match::None
}

/// Every list live in `keymap_type`, from the innermost level outwards.
///
/// The containment order of the view model, written once. An inner level
/// shadows an outer one, which is what lets a buffer kind claim a key that
/// means something else elsewhere.
fn live(keymap_type: KeymapType) -> impl Iterator<Item = &'static [Binding]> {
    buffer::bindings(keymap_type).iter().copied().chain([
        pane::BINDINGS,
        tab::BINDINGS,
        view::BINDINGS,
        program::BINDINGS,
    ])
}

/// Whether `binding` is longer than `keys` and begins with it.
fn starts_with(binding: &[KeyCombination], keys: &[KeyCombination]) -> bool {
    binding.len() > keys.len() && binding[..keys.len()] == *keys
}

/// Every binding of every keymap_type, for tests and for a help screen.
pub fn all() -> impl Iterator<Item = (KeymapType, &'static Binding)> {
    KeymapType::ALL
        .iter()
        .flat_map(|&c| live(c).flat_map(move |list| list.iter().map(move |b| (c, b))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crokey::key;

    #[test]
    fn a_single_key_resolves_to_its_action() {
        assert!(matches!(
            lookup(KeymapType::Diff(DiffLayout::SideBySide), &[key!(j)]),
            Match::Exact(_)
        ));
    }

    #[test]
    fn the_first_key_of_a_sequence_asks_for_more() {
        assert_eq!(
            lookup(KeymapType::Diff(DiffLayout::SideBySide), &[key!(g)]),
            Match::Prefix
        );
        assert!(matches!(
            lookup(
                KeymapType::Diff(DiffLayout::SideBySide),
                &[key!(g), key!(g)]
            ),
            Match::Exact(_)
        ));
    }

    #[test]
    fn an_unbound_key_matches_nothing() {
        assert_eq!(
            lookup(
                KeymapType::Diff(DiffLayout::SideBySide),
                &[key!(ctrl - alt - x)]
            ),
            Match::None
        );
        assert_eq!(
            lookup(
                KeymapType::Diff(DiffLayout::SideBySide),
                &[key!(g), key!(x)]
            ),
            Match::None
        );
    }

    #[test]
    fn a_program_binding_works_from_any_context() {
        assert!(matches!(
            lookup(KeymapType::Diff(DiffLayout::SideBySide), &[key!(q)]),
            Match::Exact(_)
        ));
    }

    #[test]
    fn no_binding_is_a_proper_prefix_of_another() {
        // The rule the whole resolver rests on. Break it and either the longer
        // binding becomes unreachable or the shorter one needs a timeout.
        //
        // Checked across the *whole chain*, not per level: a `g` bound on a
        // buffer would make a `gg` bound on the tab unreachable in that
        // buffer, silently, and only there.
        for &keymap_type in KeymapType::ALL {
            let every: Vec<_> = live(keymap_type)
                .flat_map(|l| l.iter().map(|b| b.keys))
                .collect();
            for outer in &every {
                for inner in &every {
                    assert!(
                        !starts_with(outer, inner),
                        "{keymap_type:?}: {inner:?} is a prefix of {outer:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_level_may_shadow_an_outer_one_but_not_repeat_itself() {
        // The two halves of the chain's contract.
        //
        // *Shadowing* — the same keys bound at two levels — is legal and is
        // how a buffer kind claims a key that means something else further
        // out. The inner one wins because lookup finds it first.
        //
        // *Repeating* — the same keys twice within one level — is not: there
        // is no order between them, so the second is simply unreachable.
        for &keymap_type in KeymapType::ALL {
            for list in live(keymap_type) {
                let mut seen: Vec<&[KeyCombination]> = Vec::new();
                for binding in list {
                    assert!(
                        !seen.contains(&binding.keys),
                        "{keymap_type:?}: {:?} is bound twice in one level",
                        binding.keys
                    );
                    seen.push(binding.keys);
                }
            }
        }
    }

    #[test]
    fn an_inner_level_wins_over_an_outer_one() {
        // Not provable from the tables today — nothing is shadowed yet — so it
        // is proved of the mechanism instead. Without this ordering, the
        // explorer could not bind a key the diff already uses.
        let arms: Vec<Action> = live(KeymapType::Diff(DiffLayout::SideBySide))
            .filter_map(|list| list.first().map(|b| b.action))
            .collect();
        assert!(
            matches!(arms.first(), Some(Action::Buffer(_))),
            "the focused buffer is consulted first, not {:?}",
            arms.first()
        );
        assert!(
            matches!(arms.last(), Some(Action::Program(_))),
            "the program is consulted last, not {:?}",
            arms.last()
        );
    }

    #[test]
    fn every_context_can_move() {
        // A buffer kind that forgot the motions would be unscrollable. They
        // are a shared list precisely so this cannot happen by omission.
        for &keymap_type in KeymapType::ALL {
            assert!(
                matches!(lookup(keymap_type, &[key!(j)]), Match::Exact(_)),
                "{keymap_type:?} cannot scroll"
            );
        }
    }

    #[test]
    fn a_buffer_command_is_live_only_where_it_means_something() {
        // `>` drags the divider between a side-by-side buffer's columns. A plain file has
        // no second column, so the key is simply not live there — which is
        // what stops it being a silent no-op.
        assert!(matches!(
            lookup(KeymapType::Diff(DiffLayout::SideBySide), &[key!('>')]),
            Match::Exact(_)
        ));
        assert_eq!(lookup(KeymapType::SingleFile, &[key!('>')]), Match::None);
        // The list claims the same key for the border beside it, which is a
        // different border and a different executor.
        assert_eq!(
            lookup(KeymapType::Explorer, &[key!('>')]),
            Match::Exact(Action::Tab(crate::input::TabAction::WidenLeft))
        );
    }

    #[test]
    fn every_binding_is_written_in_normalised_form() {
        // Incoming events are normalised, so an entry that is not would simply
        // never match — silently, and only for that one key.
        for (_, binding) in all() {
            for key in binding.keys {
                assert_eq!(*key, key.normalized(), "{key:?} in {:?}", binding.keys);
            }
        }
    }

    #[test]
    fn every_binding_is_reachable_by_looking_it_up() {
        for (keymap_type, binding) in all() {
            assert!(
                matches!(lookup(keymap_type, binding.keys), Match::Exact(_)),
                "{:?} is in the table but cannot be reached",
                binding.keys
            );
        }
    }
}
