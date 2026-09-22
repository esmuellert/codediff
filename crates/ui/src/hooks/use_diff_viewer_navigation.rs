//! Shared keyboard navigation for a diff viewport.

use loom::{Bubble, Listeners, Scope, ScrollHandle, use_context};

use crate::components::Ui;
use crate::keybindings::Action;

/// Connects code-view keys and wheel input using the application's configured
/// bindings and one shared scroll position.
pub fn use_diff_viewer_navigation(
    scope: &mut Scope,
    vertical: ScrollHandle,
    horizontal: ScrollHandle,
    secondary_horizontal: Option<ScrollHandle>,
) -> Listeners {
    let keybindings = use_context::<Ui>(scope).keybindings;
    let wheel_vertical = vertical.clone();
    let wheel_horizontal = horizontal.clone();
    let wheel_secondary = secondary_horizontal.clone();
    Listeners::new()
        .on_key(move |key| match key {
            key if keybindings.matches(Action::MoveDown, key) => {
                vertical.scroll_y_by(1);
                Bubble::Stop
            }
            key if keybindings.matches(Action::MoveUp, key) => {
                vertical.scroll_y_by(-1);
                Bubble::Stop
            }
            key if keybindings.matches(Action::MoveLeft, key) => {
                horizontal.scroll_x_by(-1);
                if let Some(secondary) = &secondary_horizontal {
                    secondary.scroll_x_by(-1);
                }
                Bubble::Stop
            }
            key if keybindings.matches(Action::MoveRight, key) => {
                horizontal.scroll_x_by(1);
                if let Some(secondary) = &secondary_horizontal {
                    secondary.scroll_x_by(1);
                }
                Bubble::Stop
            }
            key if keybindings.matches(Action::Start, key) => {
                horizontal.scroll_x_to_start();
                if let Some(secondary) = &secondary_horizontal {
                    secondary.scroll_x_to_start();
                }
                Bubble::Stop
            }
            key if keybindings.matches(Action::End, key) => {
                horizontal.scroll_x_to_end();
                if let Some(secondary) = &secondary_horizontal {
                    secondary.scroll_x_to_end();
                }
                Bubble::Stop
            }
            key if keybindings.matches(Action::FocusPrevious, key) => {
                loom::focus_previous();
                Bubble::Stop
            }
            _ => Bubble::Continue,
        })
        .on_wheel(move |wheel| {
            wheel_vertical.scroll_y_by(wheel.vertical.saturating_mul(3));
            wheel_horizontal.scroll_x_by(wheel.horizontal.saturating_mul(3));
            if let Some(secondary) = &wheel_secondary {
                secondary.scroll_x_by(wheel.horizontal.saturating_mul(3));
            }
            Bubble::Stop
        })
}
