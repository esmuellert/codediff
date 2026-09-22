#![doc = include_str!("../README.md")]

//! Persistent user preferences for the `codediff` application.
//!
//! This crate owns the on-disk format and does not depend on the terminal, Git,
//! or any rendering implementation. Consumers convert these values into their
//! own runtime types at the composition root.

mod path;
mod store;

pub use path::{default_path, resolve};
pub use store::{ConfigStore, Error, Result};

use serde::{Deserialize, Serialize};

/// The current on-disk schema version.
pub const CURRENT_VERSION: u32 = 1;

/// The complete user preference file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Used for future migrations.
    #[serde(default = "current_version")]
    pub version: u32,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub diff: DiffConfig,
    #[serde(default)]
    pub keybindings: Keybindings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            ui: UiConfig::default(),
            diff: DiffConfig::default(),
            keybindings: Keybindings::default(),
        }
    }
}

impl Config {
    /// Rejects values that cannot produce a useful application layout.
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.version != CURRENT_VERSION {
            return Err(format!(
                "unsupported configuration version {}",
                self.version
            ));
        }
        if self.ui.explorer_width < 8 {
            return Err("ui.explorer_width must be at least 8".to_owned());
        }
        self.keybindings.validate()
    }
}

fn current_version() -> u32 {
    CURRENT_VERSION
}

/// How two existing file versions are displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ViewLayout {
    #[default]
    SideBySide,
    Inline,
}

/// Which file-list projection opens by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ExplorerMode {
    #[default]
    Tree,
    List,
}

/// Preferences that affect the visible application layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default)]
    pub layout: ViewLayout,
    /// Whether long source lines are split into terminal lines.
    #[serde(default = "default_wrap")]
    pub wrap: bool,
    /// A built-in theme name, or `auto` for terminal detection.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Width requested for the explorer pane before its border frame.
    #[serde(default = "default_explorer_width")]
    pub explorer_width: u16,
    #[serde(default)]
    pub explorer_mode: ExplorerMode,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            layout: ViewLayout::default(),
            wrap: default_wrap(),
            theme: default_theme(),
            explorer_width: default_explorer_width(),
            explorer_mode: ExplorerMode::default(),
        }
    }
}

fn default_wrap() -> bool {
    true
}

fn default_theme() -> String {
    "auto".to_owned()
}

fn default_explorer_width() -> u16 {
    40
}

/// Diff behaviour that changes what counts as a visible change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DiffConfig {
    #[serde(default)]
    pub ignore_trim_whitespace: bool,
}

/// User-editable key aliases. The UI parses these strings into terminal keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keybindings {
    #[serde(default = "default_quit")]
    pub quit: Vec<String>,
    #[serde(default = "default_move_down")]
    pub move_down: Vec<String>,
    #[serde(default = "default_move_up")]
    pub move_up: Vec<String>,
    #[serde(default = "default_open")]
    pub open: Vec<String>,
    #[serde(default = "default_toggle_layout")]
    pub toggle_layout: Vec<String>,
    #[serde(default = "default_toggle_wrap")]
    pub toggle_wrap: Vec<String>,
    #[serde(default = "default_toggle_explorer_mode")]
    pub toggle_explorer_mode: Vec<String>,
    #[serde(default = "default_stage")]
    pub stage: Vec<String>,
    #[serde(default = "default_focus_next")]
    pub focus_next: Vec<String>,
    #[serde(default = "default_focus_previous")]
    pub focus_previous: Vec<String>,
    #[serde(default = "default_move_left")]
    pub move_left: Vec<String>,
    #[serde(default = "default_move_right")]
    pub move_right: Vec<String>,
    #[serde(default = "default_start")]
    pub start: Vec<String>,
    #[serde(default = "default_end")]
    pub end: Vec<String>,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            quit: default_quit(),
            move_down: default_move_down(),
            move_up: default_move_up(),
            open: default_open(),
            toggle_layout: default_toggle_layout(),
            toggle_wrap: default_toggle_wrap(),
            toggle_explorer_mode: default_toggle_explorer_mode(),
            stage: default_stage(),
            focus_next: default_focus_next(),
            focus_previous: default_focus_previous(),
            move_left: default_move_left(),
            move_right: default_move_right(),
            start: default_start(),
            end: default_end(),
        }
    }
}

impl Keybindings {
    fn validate(&self) -> std::result::Result<(), String> {
        let bindings = [
            ("quit", &self.quit),
            ("move_down", &self.move_down),
            ("move_up", &self.move_up),
            ("open", &self.open),
            ("toggle_layout", &self.toggle_layout),
            ("toggle_wrap", &self.toggle_wrap),
            ("toggle_explorer_mode", &self.toggle_explorer_mode),
            ("stage", &self.stage),
            ("focus_next", &self.focus_next),
            ("focus_previous", &self.focus_previous),
            ("move_left", &self.move_left),
            ("move_right", &self.move_right),
            ("start", &self.start),
            ("end", &self.end),
        ];
        for (name, values) in bindings {
            if values.is_empty() {
                return Err(format!("keybindings.{name} must not be empty"));
            }
            if values.iter().any(|value| value.trim().is_empty()) {
                return Err(format!("keybindings.{name} contains an empty key"));
            }
        }
        Ok(())
    }
}

fn default_quit() -> Vec<String> {
    vec!["q".to_owned()]
}

fn default_move_down() -> Vec<String> {
    ["j", "down"].into_iter().map(str::to_owned).collect()
}

fn default_move_up() -> Vec<String> {
    ["k", "up"].into_iter().map(str::to_owned).collect()
}

fn default_open() -> Vec<String> {
    vec!["enter".to_owned()]
}

fn default_toggle_layout() -> Vec<String> {
    vec!["t".to_owned()]
}

fn default_toggle_wrap() -> Vec<String> {
    vec!["w".to_owned()]
}

fn default_toggle_explorer_mode() -> Vec<String> {
    vec!["i".to_owned()]
}

fn default_stage() -> Vec<String> {
    vec!["space".to_owned()]
}

fn default_focus_next() -> Vec<String> {
    vec!["right".to_owned()]
}

fn default_focus_previous() -> Vec<String> {
    vec!["left".to_owned()]
}

fn default_move_left() -> Vec<String> {
    vec!["h".to_owned()]
}

fn default_move_right() -> Vec<String> {
    vec!["l".to_owned()]
}

fn default_start() -> Vec<String> {
    vec!["0".to_owned()]
}

fn default_end() -> Vec<String> {
    vec!["$".to_owned()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_current_application() {
        let config = Config::default();
        assert_eq!(config.version, CURRENT_VERSION);
        assert_eq!(config.ui.layout, ViewLayout::SideBySide);
        assert!(config.ui.wrap);
        assert_eq!(config.ui.theme, "auto");
        assert_eq!(config.ui.explorer_width, 40);
        assert_eq!(config.ui.explorer_mode, ExplorerMode::Tree);
        assert!(!config.diff.ignore_trim_whitespace);
        assert_eq!(config.keybindings.quit, ["q"]);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn partial_json_keeps_defaults_for_missing_sections() {
        let config: Config =
            serde_json::from_str(r#"{"ui":{"layout":"inline"},"keybindings":{"quit":["x"]}}"#)
                .unwrap();
        assert_eq!(config.ui.layout, ViewLayout::Inline);
        assert_eq!(config.ui.explorer_width, 40);
        assert_eq!(config.keybindings.quit, ["x"]);
        assert_eq!(config.keybindings.move_down, ["j", "down"]);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn invalid_dimensions_are_rejected() {
        let mut config = Config::default();
        config.ui.explorer_width = 7;
        assert!(config.validate().is_err());
    }
}
