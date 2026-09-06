//! Verifies that every TextMate scope rule is reachable.
//!
//! Full-table and isolated-rule passes distinguish shadowed rules from typos.

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
        read.read_colours_to_line(engine, &palette, lines.len() as u32, &lines, &mut spans);
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
        // Isolate rules that never win in the full table.
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
    // Every group must be used by at least one corpus span.
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
    let engine = Engine::new();
    assert!(matches_alone(&engine, "keyword"));
    assert!(!matches_alone(&engine, "keywrod"));
}
