//! Runtime key bindings decoded from the persistent string format.

use crokey::KeyCombination;

/// An action that can be assigned one or more keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    MoveDown,
    MoveUp,
    Open,
    ToggleLayout,
    ToggleWrap,
    ToggleExplorerMode,
    Stage,
    FocusNext,
    FocusPrevious,
    MoveLeft,
    MoveRight,
    Start,
    End,
}

/// Parsed key combinations used by the component listeners.
#[derive(Debug, Clone)]
pub struct KeyMap {
    quit: Vec<KeyCombination>,
    move_down: Vec<KeyCombination>,
    move_up: Vec<KeyCombination>,
    open: Vec<KeyCombination>,
    toggle_layout: Vec<KeyCombination>,
    toggle_wrap: Vec<KeyCombination>,
    toggle_explorer_mode: Vec<KeyCombination>,
    stage: Vec<KeyCombination>,
    focus_next: Vec<KeyCombination>,
    focus_previous: Vec<KeyCombination>,
    move_left: Vec<KeyCombination>,
    move_right: Vec<KeyCombination>,
    start: Vec<KeyCombination>,
    end: Vec<KeyCombination>,
}

impl KeyMap {
    /// Parses all configured key strings before the application takes over the
    /// terminal.
    pub fn from_config(config: &config::Keybindings) -> Result<Self, String> {
        Ok(Self {
            quit: parse(Action::Quit, &config.quit)?,
            move_down: parse(Action::MoveDown, &config.move_down)?,
            move_up: parse(Action::MoveUp, &config.move_up)?,
            open: parse(Action::Open, &config.open)?,
            toggle_layout: parse(Action::ToggleLayout, &config.toggle_layout)?,
            toggle_wrap: parse(Action::ToggleWrap, &config.toggle_wrap)?,
            toggle_explorer_mode: parse(Action::ToggleExplorerMode, &config.toggle_explorer_mode)?,
            stage: parse(Action::Stage, &config.stage)?,
            focus_next: parse(Action::FocusNext, &config.focus_next)?,
            focus_previous: parse(Action::FocusPrevious, &config.focus_previous)?,
            move_left: parse(Action::MoveLeft, &config.move_left)?,
            move_right: parse(Action::MoveRight, &config.move_right)?,
            start: parse(Action::Start, &config.start)?,
            end: parse(Action::End, &config.end)?,
        })
    }

    pub fn matches(&self, action: Action, key: KeyCombination) -> bool {
        let keys = match action {
            Action::Quit => &self.quit,
            Action::MoveDown => &self.move_down,
            Action::MoveUp => &self.move_up,
            Action::Open => &self.open,
            Action::ToggleLayout => &self.toggle_layout,
            Action::ToggleWrap => &self.toggle_wrap,
            Action::ToggleExplorerMode => &self.toggle_explorer_mode,
            Action::Stage => &self.stage,
            Action::FocusNext => &self.focus_next,
            Action::FocusPrevious => &self.focus_previous,
            Action::MoveLeft => &self.move_left,
            Action::MoveRight => &self.move_right,
            Action::Start => &self.start,
            Action::End => &self.end,
        };
        keys.contains(&key)
    }
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::from_config(&config::Keybindings::default()).expect("built-in keys are valid")
    }
}

fn parse(action: Action, values: &[String]) -> Result<Vec<KeyCombination>, String> {
    values
        .iter()
        .map(|value| crokey::parse(value).map_err(|error| format!("{action:?}: {error}")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_parse_to_the_current_aliases() {
        let keys = KeyMap::default();
        assert!(keys.matches(Action::Quit, crokey::key!(q)));
        assert!(keys.matches(Action::MoveDown, crokey::key!(j)));
        assert!(keys.matches(Action::MoveDown, crokey::key!(down)));
        assert!(keys.matches(Action::ToggleLayout, crokey::key!(t)));
    }

    #[test]
    fn invalid_configured_keys_are_reported() {
        let config = config::Keybindings {
            quit: vec!["not-a-real-key".to_owned()],
            ..Default::default()
        };
        assert!(KeyMap::from_config(&config).is_err());
    }
}
