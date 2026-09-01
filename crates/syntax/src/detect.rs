//! Language clues derived from a path and first line.

/// Inputs used to identify a file's language.
#[derive(Debug, Clone, Copy)]
pub struct Clues<'a> {
    /// A file name or path.
    pub path: &'a str,
    /// The first line, when present.
    pub first_line: Option<&'a str>,
}

impl<'a> Clues<'a> {
    pub fn new(path: &'a str, first_line: Option<&'a str>) -> Self {
        Self { path, first_line }
    }

    /// The final extension, excluding a leading dot.
    pub fn extension(&self) -> Option<&'a str> {
        let name = self.file_name();
        let stem = name.strip_prefix('.').unwrap_or(name);
        stem.rsplit_once('.').map(|(_, ext)| ext)
    }

    /// The name with any directories removed.
    pub fn file_name(&self) -> &'a str {
        self.path.rsplit(['/', '\\']).next().unwrap_or(self.path)
    }

    /// A language identified by the complete file name.
    pub fn well_known(&self) -> Option<&'static str> {
        let name = self.file_name();
        WELL_KNOWN
            .iter()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
            .map(|(_, syntax)| *syntax)
    }

    /// The interpreter named by a shebang.
    pub fn shebang(&self) -> Option<&'a str> {
        let line = self.first_line?;
        let rest = line.strip_prefix("#!")?.trim();
        let mut words = rest.split_whitespace();
        let first = words.next()?;
        let command = first.rsplit('/').next().unwrap_or(first);
        if command == "env" {
            return words.find(|word| !word.contains('='));
        }
        Some(command)
    }
}

/// Names that identify a language without an extension.
const WELL_KNOWN: &[(&str, &str)] = &[
    ("Makefile", "Makefile"),
    ("GNUmakefile", "Makefile"),
    ("Dockerfile", "Dockerfile"),
    ("Containerfile", "Dockerfile"),
    ("CMakeLists.txt", "CMake"),
    ("Cargo.lock", "TOML"),
    ("Gemfile", "Ruby"),
    ("Rakefile", "Ruby"),
    ("Vagrantfile", "Ruby"),
    (".gitignore", "Git Ignore"),
    (".gitattributes", "Git Attributes"),
    (".gitmodules", "Git Config"),
    (".editorconfig", "INI"),
    (".bashrc", "Bourne Again Shell (bash)"),
    (".zshrc", "Bourne Again Shell (bash)"),
    (".profile", "Bourne Again Shell (bash)"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn clues(path: &str) -> Clues<'_> {
        Clues::new(path, None)
    }

    #[test]
    fn an_extension_is_what_follows_the_last_dot() {
        assert_eq!(clues("src/main.rs").extension(), Some("rs"));
        assert_eq!(clues("a/b/archive.tar.gz").extension(), Some("gz"));
        assert_eq!(clues("Makefile").extension(), None);
    }

    #[test]
    fn a_leading_dot_is_a_hidden_file_not_an_extension() {
        assert_eq!(clues(".gitignore").extension(), None);
        assert_eq!(clues(".config.yaml").extension(), Some("yaml"));
    }

    #[test]
    fn a_name_can_answer_when_an_extension_cannot() {
        assert_eq!(clues("Makefile").well_known(), Some("Makefile"));
        assert_eq!(clues("deploy/Dockerfile").well_known(), Some("Dockerfile"));
        assert_eq!(clues("src/main.rs").well_known(), None);
    }

    #[test]
    fn a_name_beats_an_extension_where_both_exist() {
        assert_eq!(clues("CMakeLists.txt").well_known(), Some("CMake"));
        assert_eq!(clues("CMakeLists.txt").extension(), Some("txt"));
    }

    #[test]
    fn a_shebang_names_the_interpreter() {
        let sh = Clues::new("build", Some("#!/bin/sh"));
        assert_eq!(sh.shebang(), Some("sh"));
        let py = Clues::new("script", Some("#!/usr/bin/python3.11"));
        assert_eq!(py.shebang(), Some("python3.11"));
    }

    #[test]
    fn env_is_a_launcher_and_is_looked_through() {
        let py = Clues::new("script", Some("#!/usr/bin/env python3"));
        assert_eq!(py.shebang(), Some("python3"));
        let with_vars = Clues::new("s", Some("#!/usr/bin/env FOO=1 ruby"));
        assert_eq!(with_vars.shebang(), Some("ruby"));
    }

    #[test]
    fn an_ordinary_first_line_is_not_a_shebang() {
        assert_eq!(Clues::new("a.rs", Some("fn main() {}")).shebang(), None);
        assert_eq!(Clues::new("a.rs", None).shebang(), None);
    }

    #[test]
    fn a_windows_path_still_finds_its_name() {
        assert_eq!(clues(r"src\main.rs").file_name(), "main.rs");
        assert_eq!(clues(r"src\main.rs").extension(), Some("rs"));
    }
}
