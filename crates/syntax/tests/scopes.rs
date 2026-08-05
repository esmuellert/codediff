//! Proof that every scope in the table matches something real.
//!
//! The engine accepts a selector it cannot use. `"keywrod"` parses happily and
//! then never matches, so a typo costs a colour and says nothing — `syntax`
//! cannot catch it, and neither can the compiler. The only guard is to run
//! every selector against real source and insist it claims something.
//!
//! **Deliberately asks the matcher**, whatever the seam would choose. Every
//! selector here is a TextMate scope path, which only that engine matches; a
//! reader opening one of these files gets the parser instead. Which engine a
//! real file gets is `ui`'s `colours.rs` to say, and this says nothing about
//! it on purpose.
//!
//! Two passes, because a rule can legitimately fail to win. `keyword` is
//! outranked by `keyword.control` wherever both apply, so a whole-table pass
//! would accuse a correct rule. So: one pass with the whole table, and then,
//! for anything it did not see, a second pass with *only* that rule, where
//! nothing can outrank it. Whatever still matches nothing is dead.

use syntax::engine::scopes::SCOPES;
use syntax::{Clues, Engine, Group, Highlighted, Palette, Pen, Rule, Style};

mod corpus;

/// Every pen that appears anywhere in the corpus, under the given rules.
fn pens(engine: &Engine, rules: &[Rule]) -> Vec<Pen> {
    let palette = Palette::from_tables(rules, &[]);
    let mut seen = Vec::new();
    for (path, source) in corpus::FILES {
        let lines: Vec<String> = source.lines().map(str::to_owned).collect();
        let first = lines.first().map(String::as_str);
        let Some(grammar) = engine.find_textmate(Clues::new(path, first)) else {
            panic!("no grammar claims {path}");
        };
        let mut read = Highlighted::new(engine, grammar, &palette, &lines);
        let mut spans = Vec::new();
        read.reach(engine, &palette, lines.len() as u32, &lines, &mut spans);
        for line in &spans {
            for span in line {
                if let Some(pen) = span.style.pen
                    && !seen.contains(&pen)
                {
                    seen.push(pen);
                }
            }
        }
    }
    seen
}

/// Whether one selector, alone and so unopposed, claims anything.
fn matches_alone(engine: &Engine, selector: &'static str) -> bool {
    let rules = [Rule::new(selector, Style::pen(Pen(0)))];
    !pens(engine, &rules).is_empty()
}

#[test]
fn every_scope_in_the_table_claims_something() {
    let engine = Engine::new();
    let seen = pens(&engine, &syntax::rules());

    let mut dead = Vec::new();
    for (n, scope) in SCOPES.iter().enumerate() {
        if seen.contains(&Pen(n as u16)) {
            continue;
        }
        // Not seen in the full table. Either it is outranked everywhere, which
        // is fine, or it is a typo, which is not.
        if !matches_alone(&engine, scope.selector) {
            dead.push(scope.selector);
        }
    }
    assert!(
        dead.is_empty(),
        "these selectors match nothing in any language of the corpus, \
         so they are typos or dead scopes: {dead:#?}"
    );
}

#[test]
fn every_token_is_worn_by_something_in_the_corpus() {
    // The other half: a selector can be real and still never reach a colour,
    // if every language spells that construct some other way. A token nothing
    // wears is a colour the reader will never see.
    let engine = Engine::new();
    let seen = pens(&engine, &syntax::rules());
    let worn: Vec<Group> = seen
        .iter()
        .filter_map(|pen| SCOPES.get(pen.0 as usize).map(|s| s.group))
        .collect();

    let missing: Vec<&str> = Group::ALL
        .into_iter()
        .filter(|token| !worn.contains(token))
        .map(Group::name)
        .collect();
    assert!(
        missing.is_empty(),
        "no source in the corpus is coloured as: {missing:?}"
    );
}

#[test]
fn a_deliberate_typo_is_caught() {
    // Sabotage, so the guard above is known to be load-bearing rather than
    // merely green.
    let engine = Engine::new();
    assert!(matches_alone(&engine, "keyword"));
    assert!(!matches_alone(&engine, "keywrod"));
}
