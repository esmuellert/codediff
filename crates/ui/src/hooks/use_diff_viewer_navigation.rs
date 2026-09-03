//! Navigation shared by the views inside DiffViewer.

use loom::{Bubble, Listeners, Scope};

use super::use_horizontal_scroll::{HorizontalDimensions, HorizontalView, use_horizontal_scroll};
use super::use_scroll::{ScrollView, use_scroll};

pub fn use_diff_viewer_navigation(
    scope: &mut Scope,
    file_key: Option<&str>,
    view_line_count: u32,
    horizontal_dimensions: HorizontalDimensions,
) -> (ScrollView, HorizontalView, Listeners) {
    let (view, vertical_handle) = use_scroll(scope, file_key);
    let (horizontal, horizontal_handle) =
        use_horizontal_scroll(scope, file_key, view.width, horizontal_dimensions);

    let listeners = Listeners::new()
        .on_key(move |key| match key {
            key if key == crokey::key!(j) || key == crokey::key!(down) => {
                vertical_handle.down(view_line_count);
                Bubble::Stop
            }
            key if key == crokey::key!(k) || key == crokey::key!(up) => {
                vertical_handle.up(view_line_count);
                Bubble::Stop
            }
            key if key == crokey::key!(h) => {
                horizontal_handle.left();
                Bubble::Stop
            }
            key if key == crokey::key!(l) => {
                horizontal_handle.right();
                Bubble::Stop
            }
            key if key == crokey::key!(0) => {
                horizontal_handle.reset();
                Bubble::Stop
            }
            key if key == crokey::key!('$') => {
                horizontal_handle.end();
                Bubble::Stop
            }
            key if key == crokey::key!(left) => {
                loom::focus_previous();
                Bubble::Stop
            }
            _ => Bubble::Continue,
        })
        .on_wheel(move |delta| {
            vertical_handle.wheel(delta, view_line_count);
            Bubble::Stop
        })
        .on_mouse_down(move |mouse| {
            vertical_handle.click(mouse.local.y as u32, view_line_count);
            Bubble::Stop
        });
    (view, horizontal, listeners)
}
