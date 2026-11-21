//! Focused tests for `SetState::set` and `SetState::update`.

use std::rc::Rc;

use loom::crokey::key;
use loom::testing::Harness;
use loom::{
    Bubble, Column, ColumnProps, Listeners, Node, Scope, Text, TextProps, component, rsx, use_state,
};

#[component]
fn NumberList(scope: &mut Scope) -> Node {
    let (numbers, set_numbers) = use_state(scope, || vec![1u32, 2]);
    let text: Rc<str> = numbers
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
        .into();

    let listeners = Listeners::new()
        .on_key(move |pressed| {
            if pressed == key!(a) {
                set_numbers.update(|numbers| numbers.push(3));
                set_numbers.update(|numbers| numbers.push(4));
                Bubble::Stop
            } else if pressed == key!(r) {
                set_numbers.set(vec![10]);
                Bubble::Stop
            } else {
                Bubble::Continue
            }
        })
        .on_mouse_down(move |_| {
            set_numbers.update(Vec::clear);
            Bubble::Stop
        });

    rsx! {
        Column {
            focusable: true,
            auto_focus: true,
            listeners,
            ..,
            Text { text, .. }
        }
    }
}

#[test]
fn set_and_update_compose_and_copy_across_listeners() {
    let mut screen = Harness::new::<NumberList>(NumberListProps {}, 20, 1);
    assert_eq!(screen.screen_line(0), "1,2");
    let baseline = screen.render_count_of("NumberList");

    screen.press(key!(a));
    assert_eq!(screen.screen_line(0), "1,2,3,4");
    assert_eq!(screen.render_count_of("NumberList"), baseline + 1);

    screen.press(key!(r));
    assert_eq!(screen.screen_line(0), "10");
    assert_eq!(screen.render_count_of("NumberList"), baseline + 2);

    // Setting an equal value leaves the component clean.
    screen.press(key!(r));
    assert!(!screen.needs_draw());
    screen.draw();
    assert_eq!(screen.render_count_of("NumberList"), baseline + 2);

    screen.click(0, 0);
    assert_eq!(screen.screen_line(0), "");
}
