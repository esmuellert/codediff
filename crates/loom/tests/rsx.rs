use loom::testing::Harness;
use loom::{Node, Scope, Text, TextProps, component, rsx};

#[component]
fn Branch(scope: &mut Scope, value: u8) -> Node {
    let _ = scope;
    rsx! {
        if *value == 0 {
            "zero"
        } else if *value == 1 {
            "one"
        } else {
            "many"
        }
    }
}

#[test]
fn else_if_selects_each_branch() {
    for (value, expected) in [(0, "zero"), (1, "one"), (2, "many")] {
        let mut screen = Harness::new::<Branch>(BranchProps { value }, 10, 1);
        assert_eq!(screen.screen_line(0), expected);
    }
}

#[component]
fn Shown(scope: &mut Scope, text: String, width: u16) -> Node {
    let _ = scope;
    let text = format!("{text}:{width}");
    rsx! { Text { text: text.into(), .. } }
}

#[component]
fn Shorthand(scope: &mut Scope) -> Node {
    let _ = scope;
    let text = "named".to_owned();
    let width = 3u16;
    let key = 1usize;
    rsx! {
        Shown { key, text, width }
    }
}

#[component]
fn Longhand(scope: &mut Scope) -> Node {
    let _ = scope;
    rsx! {
        Shown { key: 1usize, text: "named".to_owned(), width: 3u16 }
    }
}

#[test]
fn a_prop_named_by_its_variable_alone_takes_that_variable() {
    let mut shorthand = Harness::new::<Shorthand>(ShorthandProps {}, 20, 1);
    let mut longhand = Harness::new::<Longhand>(LonghandProps {}, 20, 1);
    assert_eq!(shorthand.screen_line(0), "named:3");
    assert_eq!(shorthand.screen(), longhand.screen());
}
