//! Navigation shared by the views inside DiffViewer.

use file_types::DiffVersion;
use loom::{Bubble, Listeners, Scope};

use super::use_horizontal_scroll::use_horizontal_scroll;
use super::use_scroll::{ScrollView, use_scroll};

// Registered default in the pinned VS Code source.
const SCROLL_BEYOND_LAST_COLUMN: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HorizontalDimensions {
    Single {
        longest_line_cells: u32,
        gutter_cells: u16,
    },
    SideBySide {
        original_longest_line_cells: u32,
        modified_longest_line_cells: u32,
        original_gutter_cells: u16,
        modified_gutter_cells: u16,
        divider_cells: u16,
    },
}

impl HorizontalDimensions {
    fn limits(self, width: u16) -> HorizontalLimits {
        match self {
            Self::Single {
                longest_line_cells,
                gutter_cells,
            } => {
                let text_cells = u32::from(width.saturating_sub(gutter_cells));
                let maximum_first_cell = max_first_cell(longest_line_cells, text_cells);
                HorizontalLimits {
                    original: maximum_first_cell,
                    modified: maximum_first_cell,
                }
            }
            Self::SideBySide {
                original_longest_line_cells,
                modified_longest_line_cells,
                original_gutter_cells,
                modified_gutter_cells,
                divider_cells,
            } => {
                let text_cells = u32::from(
                    width
                        .saturating_sub(divider_cells)
                        .saturating_sub(original_gutter_cells)
                        .saturating_sub(modified_gutter_cells),
                );
                let original_text_cells = text_cells.div_ceil(2);
                let modified_text_cells = text_cells / 2;
                HorizontalLimits {
                    original: max_first_cell(original_longest_line_cells, original_text_cells),
                    modified: max_first_cell(modified_longest_line_cells, modified_text_cells),
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HorizontalLimits {
    original: u32,
    modified: u32,
}

impl HorizontalLimits {
    fn maximum_first_cell(self) -> u32 {
        self.original.max(self.modified)
    }

    fn view(self, first_cell: u32) -> HorizontalView {
        HorizontalView {
            requested_first_cell: first_cell,
            original_first_cell: first_cell.min(self.original),
            modified_first_cell: first_cell.min(self.modified),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HorizontalView {
    pub requested_first_cell: u32,
    original_first_cell: u32,
    modified_first_cell: u32,
}

impl HorizontalView {
    pub fn first_cell(self, version: DiffVersion) -> u32 {
        match version {
            DiffVersion::Original => self.original_first_cell,
            DiffVersion::Modified => self.modified_first_cell,
        }
    }
}

fn max_first_cell(longest_line_cells: u32, text_viewport_cells: u32) -> u32 {
    longest_line_cells
        .saturating_add(SCROLL_BEYOND_LAST_COLUMN)
        .saturating_sub(text_viewport_cells)
}

pub fn use_diff_viewer_navigation(
    scope: &mut Scope,
    file_key: Option<&str>,
    view_line_count: u32,
    horizontal_dimensions: HorizontalDimensions,
) -> (ScrollView, HorizontalView, Listeners) {
    let (view, vertical_handle) = use_scroll(scope, file_key, view_line_count);
    let horizontal_limits = horizontal_dimensions.limits(view.width);
    let (horizontal_scroll, horizontal_handle) =
        use_horizontal_scroll(scope, file_key, horizontal_limits.maximum_first_cell());
    let horizontal = horizontal_limits.view(horizontal_scroll.first_cell);

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
                horizontal_handle.scroll_by(-1);
                Bubble::Stop
            }
            key if key == crokey::key!(l) => {
                horizontal_handle.scroll_by(1);
                Bubble::Stop
            }
            key if key == crokey::key!(0) => {
                horizontal_handle.scroll_to_start();
                Bubble::Stop
            }
            key if key == crokey::key!('$') => {
                horizontal_handle.scroll_to_end();
                Bubble::Stop
            }
            key if key == crokey::key!(left) => {
                loom::focus_previous();
                Bubble::Stop
            }
            _ => Bubble::Continue,
        })
        .on_wheel(move |wheel| {
            vertical_handle.scroll_by(wheel.vertical.saturating_mul(3));
            horizontal_handle.scroll_by(wheel.horizontal.saturating_mul(3));
            Bubble::Stop
        });
    (view, horizontal, listeners)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_is_line_plus_four_cells_minus_the_viewport() {
        assert_eq!(max_first_cell(20, 10), 14);
        assert_eq!(max_first_cell(10, 20), 0);
    }

    #[test]
    fn side_by_side_limits_use_each_text_viewport() {
        let dimensions = HorizontalDimensions::SideBySide {
            original_longest_line_cells: 20,
            modified_longest_line_cells: 30,
            original_gutter_cells: 4,
            modified_gutter_cells: 5,
            divider_cells: 1,
        };

        assert_eq!(
            dimensions.limits(30),
            HorizontalLimits {
                original: 14,
                modified: 24,
            }
        );
        assert_eq!(
            dimensions.limits(31),
            HorizontalLimits {
                original: 13,
                modified: 24,
            }
        );
    }
}
