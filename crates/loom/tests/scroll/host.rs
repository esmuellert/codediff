use loom::testing::Harness;
use loom::{
    Column, ColumnProps, Layout, Node, Scope, Scroll, ScrollOffset, ScrollProps, component, rsx,
    use_scroll,
};

#[component]
fn Scrolled(scope: &mut Scope) -> Node {
    let (view, handle) = use_scroll(scope, || ScrollOffset::ZERO);
    rsx! {
        Scroll {
            handle: Some(handle),
            view: view,
            layout: Layout { grow: 1, ..Default::default() },
            ..,
            Column {
                "abcdef"
                "second"
                "third"
            }
        }
    }
}

#[test]
fn a_scroll_host_moves_one_content_tree_in_two_dimensions() {
    let mut screen = Harness::new::<Scrolled>(ScrolledProps {}, 4, 2);
    assert_eq!(screen.screen(), vec!["abcd", "seco"]);

    screen.wheel_horizontal(0, 0, 1);
    assert!(screen.needs_draw());
    assert_eq!(screen.screen(), vec!["bcde", "econ"]);

    screen.wheel(0, 0, 1);
    assert_eq!(screen.screen(), vec!["econ", "hird"]);
}
