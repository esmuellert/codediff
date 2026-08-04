//! Turning a stream of keypresses into commands.
//!
//! This is the only stateful part of the keymap, and it exists because **a key
//! is not a command**: `g` alone means nothing, `5` alone means nothing.
//! Something has to remember what came before, and this is that something.
//!
//! It **resolves and returns**; it never acts. What comes out goes back to the
//! event loop, which is the only thing that can see all three kinds of
//! answerer. That is what lets [`Resolver`] hold no references at all, and be
//! a pure function of its own two fields plus one key — so a test is a string
//! of keys. `cargo xtask lint-arch` refuses a clock in this directory for the
//! same reason.

use std::num::NonZeroU32;

use crokey::KeyCombination;
use crokey::key;

use crate::input::command::Command;
use crate::input::keymap::{self, KeymapType, Match};

/// Counts above this are treated as this.
///
/// Not a limit anyone will meet on purpose; it stops a reader who fell asleep
/// on the `9` key from asking for four billion of anything.
const MAX_COUNT: u32 = 100_000;

/// What one key did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// A count digit, or a proper prefix of a binding. More keys are needed.
    Pending,
    /// Cleared a sequence or count in progress. Nothing runs.
    Cancelled,
    /// Nothing is bound to this. Nothing runs, and anything in progress is
    /// dropped.
    Unbound,
    Run(Command),
}

/// The pending sequence and count.
///
/// Everything the keymap remembers between keypresses is these two fields.
#[derive(Debug, Clone, Default)]
pub struct Resolver {
    keys: Vec<KeyCombination>,
    count: u32,
}

impl Resolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// The keys typed so far that have not resolved, for a `showcmd` display.
    pub fn pending(&self) -> &[KeyCombination] {
        &self.keys
    }

    pub fn count(&self) -> Option<NonZeroU32> {
        NonZeroU32::new(self.count)
    }

    /// Feeds one key in.
    pub fn key(&mut self, key: KeyCombination, keymap_type: KeymapType) -> Resolution {
        // Terminals disagree about whether shift is reported as a modifier or
        // by the character's case; normalising both sides is what makes them
        // meet.
        let key = key.normalized();

        // Escape takes back whatever is in flight, and only then. With nothing
        // in flight it falls through to the table, where it quits.
        if key == key!(esc) && self.in_flight() {
            self.reset();
            return Resolution::Cancelled;
        }

        if let Some(digit) = self.count_digit(key) {
            self.count = (self.count.saturating_mul(10).saturating_add(digit)).min(MAX_COUNT);
            return Resolution::Pending;
        }

        self.keys.push(key);
        match keymap::lookup(keymap_type, &self.keys) {
            Match::Exact(action) => {
                let count = self.count();
                self.reset();
                Resolution::Run(Command { count, action })
            }
            Match::Prefix => Resolution::Pending,
            Match::None => {
                self.reset();
                Resolution::Unbound
            }
        }
    }

    /// The digit this key contributes to a count, if it is one.
    ///
    /// Two rules, both vim's. Digits count only when no sequence is in
    /// progress, so `g5` is a sequence rather than `g` then a count. And `0`
    /// counts only once a count has started, because it is also a motion —
    /// the single point at which counts and bindings interact.
    fn count_digit(&self, key: KeyCombination) -> Option<u32> {
        if !self.keys.is_empty() {
            return None;
        }
        let digit = key.as_letter()?.to_digit(10)?;
        (digit > 0 || self.count > 0).then_some(digit)
    }

    fn in_flight(&self) -> bool {
        !self.keys.is_empty() || self.count > 0
    }

    fn reset(&mut self) {
        self.keys.clear();
        self.count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::buffer::{BufferAction, Motion};
    use crate::input::command::Action;
    use crate::input::program::ProgramAction;
    use align::DiffLayout;

    /// Feeds a line of keys in and returns what came out.
    ///
    /// The whole point of the resolver being pure: a test is a string.
    fn run(keys: &[KeyCombination]) -> Vec<Resolution> {
        let mut input = Resolver::new();
        keys.iter()
            .map(|k| input.key(*k, KeymapType::Diff(DiffLayout::SideBySide)))
            .collect()
    }

    fn last(keys: &[KeyCombination]) -> Resolution {
        *run(keys).last().expect("at least one key")
    }

    fn command(keys: &[KeyCombination]) -> Command {
        match last(keys) {
            Resolution::Run(command) => command,
            other => panic!("expected a command, got {other:?}"),
        }
    }

    #[test]
    fn a_single_key_resolves_at_once() {
        assert_eq!(
            command(&[key!(j)]).action,
            Action::Buffer(BufferAction::Motion(Motion::Down))
        );
    }

    #[test]
    fn a_sequence_waits_for_its_second_key() {
        assert_eq!(
            run(&[key!(g), key!(g)]),
            [
                Resolution::Pending,
                Resolution::Run(Command::new(Action::Buffer(BufferAction::Motion(
                    Motion::Top
                ))))
            ]
        );
    }

    #[test]
    fn an_unfinished_sequence_followed_by_nonsense_runs_nothing() {
        assert_eq!(
            run(&[key!(g), key!(x)]),
            [Resolution::Pending, Resolution::Unbound]
        );
    }

    #[test]
    fn a_sequence_does_not_leak_into_the_next_key() {
        // The bug the old `pending_g: bool` invited: `g` then `x` then `j`
        // must still be a plain `j`.
        let outcomes = run(&[key!(g), key!(x), key!(j)]);
        assert_eq!(
            outcomes[2],
            Resolution::Run(Command::new(Action::Buffer(BufferAction::Motion(
                Motion::Down
            ))))
        );
    }

    #[test]
    fn escape_takes_back_a_pending_sequence_instead_of_quitting() {
        // Without this, pressing `g` and changing your mind would exit.
        assert_eq!(
            run(&[key!(g), key!(esc)]),
            [Resolution::Pending, Resolution::Cancelled]
        );
    }

    #[test]
    fn escape_with_nothing_in_flight_still_quits() {
        assert_eq!(
            command(&[key!(esc)]).action,
            Action::Program(ProgramAction::Quit)
        );
    }

    #[test]
    fn digits_accumulate_into_a_count() {
        let command = command(&[key!('1'), key!('2'), key!(j)]);
        assert_eq!(command.repeat(), 12);
        assert_eq!(
            command.action,
            Action::Buffer(BufferAction::Motion(Motion::Down))
        );
    }

    #[test]
    fn zero_is_a_motion_until_a_count_has_started() {
        // vim's rule, and the only place counts and bindings meet.
        assert_eq!(
            command(&[key!('0')]).action,
            Action::Buffer(BufferAction::Motion(Motion::ScrollHome))
        );

        let command = command(&[key!('5'), key!('0'), key!(j)]);
        assert_eq!(command.repeat(), 50);
    }

    #[test]
    fn a_count_is_dropped_along_with_the_key_that_failed() {
        let outcomes = run(&[key!('5'), key!(ctrl - alt - x), key!(j)]);
        assert_eq!(outcomes[1], Resolution::Unbound);
        assert_eq!(
            outcomes[2],
            Resolution::Run(Command::new(Action::Buffer(BufferAction::Motion(
                Motion::Down
            )))),
            "the 5 must not survive"
        );
    }

    #[test]
    fn escape_takes_back_a_count_too() {
        assert_eq!(
            run(&[key!('5'), key!(esc)]),
            [Resolution::Pending, Resolution::Cancelled]
        );
    }

    #[test]
    fn digits_are_keys_again_once_a_sequence_is_in_progress() {
        // `g5` is a sequence that happens not to exist, not `g` then a count.
        assert_eq!(
            run(&[key!(g), key!('5')]),
            [Resolution::Pending, Resolution::Unbound]
        );
    }

    #[test]
    fn a_count_applies_to_a_sequence() {
        assert_eq!(command(&[key!('5'), key!(g), key!(g)]).repeat(), 5);
    }

    #[test]
    fn an_absurd_count_is_capped_rather_than_wrapping() {
        let keys: Vec<_> = std::iter::repeat_n(key!('9'), 40)
            .chain([key!(j)])
            .collect();
        assert_eq!(command(&keys).repeat(), MAX_COUNT);
    }

    #[test]
    fn nothing_is_left_behind_after_a_command_runs() {
        let mut input = Resolver::new();
        input.key(key!('5'), KeymapType::Diff(DiffLayout::SideBySide));
        input.key(key!(g), KeymapType::Diff(DiffLayout::SideBySide));
        input.key(key!(g), KeymapType::Diff(DiffLayout::SideBySide));
        assert!(input.pending().is_empty());
        assert_eq!(input.count(), None);
    }

    #[test]
    fn the_pending_keys_are_visible_while_waiting() {
        let mut input = Resolver::new();
        input.key(key!('1'), KeymapType::Diff(DiffLayout::SideBySide));
        assert_eq!(input.count().map(NonZeroU32::get), Some(1));
        input.key(key!(g), KeymapType::Diff(DiffLayout::SideBySide));
        assert_eq!(input.pending(), [key!(g)]);
    }
}
