use loom::testing::Harness;
use loom::{Node, Scope, component, rsx};

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
        assert_eq!(screen.screen_row(0), expected);
    }
}
