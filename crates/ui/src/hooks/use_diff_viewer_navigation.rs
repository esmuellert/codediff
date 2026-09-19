//! Shared navigation and geometry for the views inside DiffViewer.

use file_types::DiffVersion;
use loom::{Bubble, Listeners, Scope, use_context};

use crate::components::Ui;
use crate::keybindings::Action;

use super::use_horizontal_scroll::HorizontalHandle;
use super::use_scroll::ScrollHandle;

// Keep four cells beyond the longest line.
const SCROLL_BEYOND_LAST_COLUMN: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HorizontalDimensions {
    Single {
        longest_line_cells: u32,
        gutter_cells: u16,
    },
    Inline {
        longest_line_cells: u32,
        original_gutter_cells: u16,
        modified_gutter_cells: u16,
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
    pub(crate) fn limits(self, width: u16) -> HorizontalLimits {
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
            Self::Inline {
                longest_line_cells,
                original_gutter_cells,
                modified_gutter_cells,
            } => {
                let text_cells = u32::from(
                    width
                        .saturating_sub(original_gutter_cells)
                        .saturating_sub(modified_gutter_cells),
                );
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
pub(crate) struct HorizontalLimits {
    original: u32,
    modified: u32,
}

impl HorizontalLimits {
    pub(crate) fn maximum_first_cell(self) -> u32 {
        self.original.max(self.modified)
    }

    pub(crate) fn view(self, first_cell: u32) -> HorizontalView {
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
    if longest_line_cells <= text_viewport_cells {
        0
    } else {
        longest_line_cells
            .saturating_add(SCROLL_BEYOND_LAST_COLUMN)
            .saturating_sub(text_viewport_cells)
    }
}

/// Connects code-view input using the application's configured keys.
pub fn use_diff_viewer_navigation(
    scope: &mut Scope,
    vertical_handle: ScrollHandle,
    horizontal_handle: HorizontalHandle,
) -> Listeners {
    let keybindings = use_context::<Ui>(scope).keybindings;
    Listeners::new()
        .on_key(move |key| match key {
            key if keybindings.matches(Action::MoveDown, key) => {
                vertical_handle.scroll_by(1);
                Bubble::Stop
            }
            key if keybindings.matches(Action::MoveUp, key) => {
                vertical_handle.scroll_by(-1);
                Bubble::Stop
            }
            key if keybindings.matches(Action::MoveLeft, key) => {
                horizontal_handle.scroll_by(-1);
                Bubble::Stop
            }
            key if keybindings.matches(Action::MoveRight, key) => {
                horizontal_handle.scroll_by(1);
                Bubble::Stop
            }
            key if keybindings.matches(Action::Start, key) => {
                horizontal_handle.scroll_to_start();
                Bubble::Stop
            }
            key if keybindings.matches(Action::End, key) => {
                horizontal_handle.scroll_to_end();
                Bubble::Stop
            }
            key if keybindings.matches(Action::FocusPrevious, key) => {
                loom::focus_previous();
                Bubble::Stop
            }
            _ => Bubble::Continue,
        })
        .on_wheel(move |wheel| {
            vertical_handle.scroll_by(wheel.vertical.saturating_mul(3));
            horizontal_handle.scroll_by(wheel.horizontal.saturating_mul(3));
            Bubble::Stop
        })
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
    fn inline_limit_uses_one_text_viewport_after_both_gutters() {
        let dimensions = HorizontalDimensions::Inline {
            longest_line_cells: 30,
            original_gutter_cells: 4,
            modified_gutter_cells: 5,
        };

        assert_eq!(
            dimensions.limits(20),
            HorizontalLimits {
                original: 23,
                modified: 23,
            }
        );
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
