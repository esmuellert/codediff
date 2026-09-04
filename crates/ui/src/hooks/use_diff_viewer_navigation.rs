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
    let (view, vertical_handle) = use_scroll(scope, file_key, view_line_count);
    let (horizontal, horizontal_handle) =
        use_horizontal_scroll(scope, file_key, view.width, horizontal_dimensions);

    let listeners = Listeners::new()
        .on_key(move |key| match key {
            key if key == crokey::key!(j) || key == crokey::key!(down) => {
                vertical_handle.scroll_by(1);
                Bubble::Stop
            }
            key if key == crokey::key!(k) || key == crokey::key!(up) => {
                vertical_handle.scroll_by(-1);
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
            vertical_handle.scroll_by(delta.saturating_mul(3));
            Bubble::Stop
        });
    (view, horizontal, listeners)
}
