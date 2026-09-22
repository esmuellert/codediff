//! Shared keyboard navigation for a diff viewport.

use loom::{Bubble, Listeners, Scope, ScrollHandle, use_context};

use crate::components::Ui;
use crate::keybindings::Action;

/// Connects code-view keys and wheel input using the application's configured
/// bindings and one shared scroll position.
pub fn use_diff_viewer_navigation(scope: &mut Scope, scroll: ScrollHandle) -> Listeners {
    let keybindings = use_context::<Ui>(scope).keybindings;
    let wheel_scroll = scroll.clone();
    Listeners::new()
        .on_key(move |key| match key {
            key if keybindings.matches(Action::MoveDown, key) => {
                scroll.scroll_y_by(1);
                Bubble::Stop
            }
            key if keybindings.matches(Action::MoveUp, key) => {
                scroll.scroll_y_by(-1);
                Bubble::Stop
            }
            key if keybindings.matches(Action::MoveLeft, key) => {
                scroll.scroll_x_by(-1);
                Bubble::Stop
            }
            key if keybindings.matches(Action::MoveRight, key) => {
                scroll.scroll_x_by(1);
                Bubble::Stop
            }
            key if keybindings.matches(Action::Start, key) => {
                scroll.scroll_x_to_start();
                Bubble::Stop
            }
            key if keybindings.matches(Action::End, key) => {
                scroll.scroll_x_to_end();
                Bubble::Stop
            }
            key if keybindings.matches(Action::FocusPrevious, key) => {
                loom::focus_previous();
                Bubble::Stop
            }
            _ => Bubble::Continue,
        })
        .on_wheel(move |wheel| {
            wheel_scroll.scroll_by(
                wheel.horizontal.saturating_mul(3),
                wheel.vertical.saturating_mul(3),
            );
            Bubble::Stop
        })
}
