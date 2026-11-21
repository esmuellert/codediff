use loom::testing::Harness;
use loom::{
    Column, ColumnProps, Layout, Listeners, Node, Scope, Scroll, ScrollOffset, ScrollProps,
    component, rsx, use_scroll,
};

#[component]
fn Scrolled(scope: &mut Scope) -> Node {
    let (view, handle) = use_scroll(scope, || ScrollOffset::ZERO);
    rsx! {
        Scroll {
            handle: Some(handle),
            view,
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

#[component]
fn TwoListenersScroll(scope: &mut Scope) -> Node {
    let (view, scroll) = use_scroll(scope, || ScrollOffset::ZERO);
    let listeners = Listeners::new()
        .on_key(move |pressed| {
            if pressed == loom::crokey::key!(j) {
                scroll.scroll_y_by(1);
                loom::Bubble::Stop
            } else {
                loom::Bubble::Continue
            }
        })
        .on_mouse_down(move |_| {
            scroll.scroll_to_end();
            loom::Bubble::Stop
        });

    rsx! {
        Column {
            focusable: true,
            auto_focus: true,
            listeners,
            layout: Layout { grow: 1, ..Default::default() },
            ..,
            Scroll {
                view,
                handle: Some(scroll),
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                Column {
                    "first"
                    "second"
                    "third"
                    "fourth"
                }
            }
        }
    }
}

#[test]
fn scroll_handle_is_used_in_two_listeners_without_cloning() {
    let mut screen = Harness::new::<TwoListenersScroll>(TwoListenersScrollProps {}, 10, 2);
    screen.force_draw().force_draw();
    assert_eq!(screen.screen(), vec!["first", "second"]);

    screen.press(loom::crokey::key!(j)).force_draw();
    assert_eq!(screen.screen(), vec!["second", "third"]);

    screen.click(0, 0).force_draw();
    assert_eq!(screen.screen(), vec!["third", "fourth"]);
}
