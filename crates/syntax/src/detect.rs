//! Which language a file is written in.
//!
//! Three questions in order, cheapest first: the file name, then the
//! extension, then the first line. Names before extensions because `Makefile`
//! and `Dockerfile` have none, and `CMakeLists.txt` would otherwise be read as
//! plain text.
//!
//! The first line matters more here than in an editor. A file called `build`
//! with `#!/usr/bin/env bash` at the top is a shell script, and a diff viewer
//! meets those constantly — `delta` cannot ask, because it is handed a diff
//! rather than a file, and has an open request for exactly this.

/// What identifies a file's language, before an engine is involved.
///
/// The name and the first line, which is everything the two lookups need. A
/// borrowed view, because a caller already owns both.
#[derive(Debug, Clone, Copy)]
pub struct Clues<'a> {
    /// The file's name, with or without directories — `src/main.rs` or
    /// `main.rs` both work.
    pub path: &'a str,
    /// The first line, if the file has one. A shebang, an XML declaration or a
    /// modeline lives here.
    pub first_line: Option<&'a str>,
}

impl<'a> Clues<'a> {
    pub fn new(path: &'a str, first_line: Option<&'a str>) -> Self {
        Self { path, first_line }
    }

    /// The part after the last dot, if there is one.
    ///
    /// Only the last: `archive.tar.gz` is a `gz`, and `.gitignore` has no
    /// extension at all rather than an extension of `gitignore`, which is why
    /// the leading dot is skipped before looking.
    pub fn extension(&self) -> Option<&'a str> {
        let name = self.file_name();
        let stem = name.strip_prefix('.').unwrap_or(name);
        stem.rsplit_once('.').map(|(_, ext)| ext)
    }

    /// The name with any directories removed.
    pub fn file_name(&self) -> &'a str {
        // Both separators, because a repository path is always `/` but a
        // caller on Windows may hand us the other one.
        self.path.rsplit(['/', '\\']).next().unwrap_or(self.path)
    }

    /// A file whose *name* names its language, extension or not.
    ///
    /// Checked before the extension so that `CMakeLists.txt` is CMake rather
    /// than text.
    pub fn well_known(&self) -> Option<&'static str> {
        let name = self.file_name();
        WELL_KNOWN
            .iter()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
            .map(|(_, syntax)| *syntax)
    }

    /// The interpreter a shebang names, if the first line is one.
    ///
    /// `#!/usr/bin/env python3` gives `python3`; `#!/bin/sh` gives `sh`. The
    /// engine is left to decide what that means, since it knows which names
    /// its grammars answer to.
    pub fn shebang(&self) -> Option<&'a str> {
        let line = self.first_line?;
        let rest = line.strip_prefix("#!")?.trim();
        let mut words = rest.split_whitespace();
        let first = words.next()?;
        let command = first.rsplit('/').next().unwrap_or(first);
        // `env` is a launcher, not a language: the interpreter is the next
        // word, skipping any `VAR=value` assignments it was given.
        if command == "env" {
            return words.find(|word| !word.contains('='));
        }
        Some(command)
    }
}

/// Files whose name is the whole answer.
///
/// Kept short on purpose: this is only for names an extension lookup cannot
/// reach. Anything with a useful extension belongs to the engine's own table,
/// which knows far more of them than we could maintain.
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
        // `.gitignore` is a name, not an extension of `gitignore`.
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
        // The point of checking names first: this is CMake, not plain text.
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
