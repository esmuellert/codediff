//! Navigation shared by the views inside DiffViewer.

use loom::{Bubble, Listeners, Scope};

use super::use_scroll::{ScrollView, use_scroll};

pub fn use_diff_viewer_navigation(
    scope: &mut Scope,
    file_key: Option<&str>,
    view_line_count: u32,
) -> (ScrollView, Listeners) {
    let (view, handle) = use_scroll(scope, file_key);
    let listeners = Listeners::new()
        .on_key(move |key| match key {
            key if key == crokey::key!(j) || key == crokey::key!(down) => {
                handle.down(view_line_count);
                Bubble::Stop
            }
            key if key == crokey::key!(k) || key == crokey::key!(up) => {
                handle.up(view_line_count);
                Bubble::Stop
            }
            key if key == crokey::key!(left) => {
                loom::focus_previous();
                Bubble::Stop
            }
            _ => Bubble::Continue,
        })
        .on_wheel(move |delta| {
            handle.wheel(delta, view_line_count);
            Bubble::Stop
        })
        .on_mouse_down(move |mouse| {
            handle.click(mouse.local.y as u32, view_line_count);
            Bubble::Stop
        });
    (view, listeners)
}
