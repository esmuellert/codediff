//! Every language the parser knows, and the rules we add to their queries.
//!
//! Data, not behaviour: a table and the handful of one-line query additions
//! that go with it. Adding a language is a dependency and a row here, and
//! nothing in [`super`] changes.
//!
//! The rows are not uniform because the crates are not: the query constant is
//! `HIGHLIGHT_QUERY` in some and `HIGHLIGHTS_QUERY` in others, injections and
//! locals are present or absent per crate, and TypeScript and PHP each expose
//! two languages. Writing that out is duller than a macro and survives the
//! next crate that is different again.

use tree_sitter::Language;

/// One language we can parse.
///
/// `language` is a function rather than a value because a `Language` is
/// produced by a call, so a table of them cannot be a constant.
pub struct Parser {
    /// What the injection queries call it. `injection.language` in one
    /// grammar's query is matched against this, which is how a fenced code
    /// block in Markdown finds the grammar for its language.
    pub name: &'static str,
    pub language: fn() -> Language,
    /// The highlight queries, joined in order.
    ///
    /// Usually one. More when a crate splits a dialect into a second file
    /// (JavaScript's JSX), and — the case that matters — when a grammar's
    /// query is an **increment on another language's rather than a whole
    /// one**. TypeScript's ships five captures and C++'s ships six, because
    /// upstream expects the tool to compose them with JavaScript's and C's;
    /// the crates carry no marker saying so, and the symptom is a file that
    /// comes back entirely plain. The derived language goes first, because an
    /// earlier pattern wins.
    pub highlights: &'static [&'static str],
    pub injections: &'static str,
    pub locals: &'static str,
    /// Extensions, lower case and without the dot.
    pub extensions: &'static [&'static str],
    /// Whole file names, for the files that have no extension.
    pub file_names: &'static [&'static str],
    /// Interpreters, matched against a `#!` line.
    pub shebangs: &'static [&'static str],
}

/// Rules appended after a grammar's own.
///
/// **Appended, never inserted.** Where two patterns cover one node this engine
/// resolves in favour of the *later*, which is why a shipped query sometimes
/// loses its own more specific rule — JSON captures keys and then captures
/// every string, so the key rule never wins. Appending is how a rule wins, and
/// it is what Helix and nvim-treesitter do at much greater length by forking
/// the whole file.
///
/// Each of these is one line, exists because the matcher already gets that
/// case right, and would be a visible regression to lose. A malformed one
/// fails to compile and `every_language_in_the_table_compiles_its_query`
/// catches it.
mod overrides {
    /// The shipped query captures the key, then captures every string.
    pub const JSON: &str = "(pair key: (_) @string.special.key)";

    /// `(field_identifier) @property` comes after the method rule and takes
    /// every method name with it.
    pub const GO: &str = "(method_declaration name: (field_identifier) @function.method)";

    /// A decorator is scoped as the function it calls. Catppuccin gives
    /// annotations a colour of their own, and the matcher already does.
    ///
    /// The rule has to name the *inner* node: where one capture is nested
    /// inside another, the inner one is the more specific claim and wins, so
    /// capturing the whole `decorator` would be overruled by the rule on the
    /// identifier inside it.
    pub const PYTHON: &str = r#"
        (decorator (identifier) @attribute)
        (decorator (attribute) @attribute)
        ((identifier) @variable.builtin (#any-of? @variable.builtin "self" "cls"))
    "#;
    pub const TYPESCRIPT: &str = r#"
        (decorator (identifier) @attribute)
        (decorator (call_expression function: (identifier) @attribute))
    "#;

    /// The delimiters of a regular expression belong to it. Without this the
    /// slashes are captured as division operators, which is what they are
    /// everywhere else.
    pub const JAVASCRIPT: &str = r#"(regex "/" @string.special)"#;

    /// A character literal is a number in C's query and a string in Rust's.
    /// It is neither, and a theme that parts them has nowhere to say so.
    pub const C: &str = "(char_literal) @character";
    pub const RUST: &str = "(char_literal) @character";
}

/// Every language we parse.
///
/// Adding one is a dependency and a row. The rows are not uniform because the
/// crates are not: the query constant is `HIGHLIGHT_QUERY` in some and
/// `HIGHLIGHTS_QUERY` in others, injections and locals are present or absent
/// per crate, and TypeScript and PHP each expose two languages. Writing that
/// out is duller than a macro and survives the next crate that is different
/// again.
///
/// **Order is meaning.** A [`Grammar`] is an index into this, so rows may be
/// added or edited but are read by position at runtime — which is fine,
/// because the same build produces both ends.
pub static LANGUAGES: &[Parser] = &[
    Parser {
        name: "rust",
        language: || tree_sitter_rust::LANGUAGE.into(),
        highlights: &[tree_sitter_rust::HIGHLIGHTS_QUERY, overrides::RUST],
        injections: tree_sitter_rust::INJECTIONS_QUERY,
        locals: "",
        extensions: &["rs"],
        file_names: &[],
        shebangs: &[],
    },
    Parser {
        name: "python",
        language: || tree_sitter_python::LANGUAGE.into(),
        highlights: &[tree_sitter_python::HIGHLIGHTS_QUERY, overrides::PYTHON],
        injections: "",
        locals: "",
        extensions: &["py", "pyi", "pyw"],
        file_names: &[],
        shebangs: &["python", "python2", "python3"],
    },
    Parser {
        name: "javascript",
        language: || tree_sitter_javascript::LANGUAGE.into(),
        // JSX is not a separate grammar in JavaScript, only extra rules.
        highlights: &[
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
            overrides::JAVASCRIPT,
        ],
        injections: tree_sitter_javascript::INJECTIONS_QUERY,
        locals: tree_sitter_javascript::LOCALS_QUERY,
        extensions: &["js", "jsx", "mjs", "cjs"],
        file_names: &[],
        shebangs: &["node"],
    },
    Parser {
        name: "typescript",
        language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        // TypeScript is JavaScript plus types, and so is its query.
        highlights: &[
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            overrides::JAVASCRIPT,
            overrides::TYPESCRIPT,
        ],
        injections: "",
        locals: tree_sitter_typescript::LOCALS_QUERY,
        extensions: &["ts", "mts", "cts"],
        file_names: &[],
        shebangs: &["ts-node", "deno", "bun"],
    },
    Parser {
        // A separate grammar rather than extra rules, because TSX and
        // TypeScript disagree about what `<T>` means.
        name: "tsx",
        language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
        highlights: &[
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
            overrides::JAVASCRIPT,
            overrides::TYPESCRIPT,
        ],
        injections: "",
        locals: tree_sitter_typescript::LOCALS_QUERY,
        extensions: &["tsx"],
        file_names: &[],
        shebangs: &[],
    },
    Parser {
        name: "go",
        language: || tree_sitter_go::LANGUAGE.into(),
        highlights: &[tree_sitter_go::HIGHLIGHTS_QUERY, overrides::GO],
        injections: "",
        locals: "",
        extensions: &["go"],
        file_names: &[],
        shebangs: &[],
    },
    Parser {
        name: "java",
        language: || tree_sitter_java::LANGUAGE.into(),
        highlights: &[tree_sitter_java::HIGHLIGHTS_QUERY],
        injections: "",
        locals: "",
        extensions: &["java"],
        file_names: &[],
        shebangs: &[],
    },
    Parser {
        name: "c",
        language: || tree_sitter_c::LANGUAGE.into(),
        highlights: &[tree_sitter_c::HIGHLIGHT_QUERY, overrides::C],
        injections: "",
        locals: "",
        extensions: &["c", "h"],
        file_names: &[],
        shebangs: &[],
    },
    Parser {
        name: "cpp",
        language: || tree_sitter_cpp::LANGUAGE.into(),
        // C++ is C plus classes, and so is its query.
        highlights: &[
            tree_sitter_cpp::HIGHLIGHT_QUERY,
            tree_sitter_c::HIGHLIGHT_QUERY,
            overrides::C,
        ],
        injections: "",
        locals: "",
        extensions: &["cc", "cpp", "cxx", "hpp", "hh", "hxx", "c++", "h++"],
        file_names: &[],
        shebangs: &[],
    },
    Parser {
        name: "c_sharp",
        language: || tree_sitter_c_sharp::LANGUAGE.into(),
        highlights: &[tree_sitter_c_sharp::HIGHLIGHTS_QUERY],
        injections: "",
        locals: "",
        extensions: &["cs"],
        file_names: &[],
        shebangs: &[],
    },
    Parser {
        name: "ruby",
        language: || tree_sitter_ruby::LANGUAGE.into(),
        highlights: &[tree_sitter_ruby::HIGHLIGHTS_QUERY],
        injections: "",
        locals: tree_sitter_ruby::LOCALS_QUERY,
        extensions: &["rb", "rake", "gemspec", "ru"],
        file_names: &["Gemfile", "Rakefile", "Guardfile", "Podfile"],
        shebangs: &["ruby"],
    },
    Parser {
        name: "php",
        language: || tree_sitter_php::LANGUAGE_PHP.into(),
        highlights: &[tree_sitter_php::HIGHLIGHTS_QUERY],
        injections: tree_sitter_php::INJECTIONS_QUERY,
        locals: "",
        extensions: &["php", "phtml"],
        file_names: &[],
        shebangs: &["php"],
    },
    Parser {
        name: "bash",
        language: || tree_sitter_bash::LANGUAGE.into(),
        highlights: &[tree_sitter_bash::HIGHLIGHT_QUERY],
        injections: "",
        locals: "",
        extensions: &["sh", "bash", "zsh", "ksh", "bashrc", "zshrc", "profile"],
        file_names: &[".bashrc", ".zshrc", ".bash_profile", ".profile"],
        shebangs: &["sh", "bash", "zsh", "ksh", "dash"],
    },
    Parser {
        name: "json",
        language: || tree_sitter_json::LANGUAGE.into(),
        highlights: &[tree_sitter_json::HIGHLIGHTS_QUERY, overrides::JSON],
        injections: "",
        locals: "",
        extensions: &["json", "jsonc", "webmanifest"],
        file_names: &[".babelrc", ".eslintrc", ".prettierrc"],
        shebangs: &[],
    },
    Parser {
        name: "yaml",
        language: || tree_sitter_yaml::LANGUAGE.into(),
        highlights: &[tree_sitter_yaml::HIGHLIGHTS_QUERY],
        injections: "",
        locals: "",
        extensions: &["yaml", "yml"],
        file_names: &[],
        shebangs: &[],
    },
    Parser {
        name: "toml",
        language: || tree_sitter_toml_ng::LANGUAGE.into(),
        highlights: &[tree_sitter_toml_ng::HIGHLIGHTS_QUERY],
        injections: "",
        locals: "",
        extensions: &["toml"],
        file_names: &["Cargo.lock", "Pipfile", "poetry.lock"],
        shebangs: &[],
    },
    Parser {
        name: "css",
        language: || tree_sitter_css::LANGUAGE.into(),
        highlights: &[tree_sitter_css::HIGHLIGHTS_QUERY],
        injections: "",
        locals: "",
        extensions: &["css"],
        file_names: &[],
        shebangs: &[],
    },
    Parser {
        name: "html",
        language: || tree_sitter_html::LANGUAGE.into(),
        highlights: &[tree_sitter_html::HIGHLIGHTS_QUERY],
        // `<script>` and `<style>` reach JavaScript and CSS through this.
        injections: tree_sitter_html::INJECTIONS_QUERY,
        locals: "",
        extensions: &["html", "htm", "xhtml"],
        file_names: &[],
        shebangs: &[],
    },
    Parser {
        name: "lua",
        language: || tree_sitter_lua::LANGUAGE.into(),
        highlights: &[tree_sitter_lua::HIGHLIGHTS_QUERY],
        injections: tree_sitter_lua::INJECTIONS_QUERY,
        locals: tree_sitter_lua::LOCALS_QUERY,
        extensions: &["lua"],
        file_names: &[],
        shebangs: &["lua"],
    },
    Parser {
        name: "scala",
        language: || tree_sitter_scala::LANGUAGE.into(),
        highlights: &[tree_sitter_scala::HIGHLIGHTS_QUERY],
        injections: "",
        locals: tree_sitter_scala::LOCALS_QUERY,
        extensions: &["scala", "sc", "sbt"],
        file_names: &[],
        shebangs: &[],
    },
    Parser {
        name: "swift",
        language: || tree_sitter_swift::LANGUAGE.into(),
        highlights: &[tree_sitter_swift::HIGHLIGHTS_QUERY],
        injections: tree_sitter_swift::INJECTIONS_QUERY,
        locals: tree_sitter_swift::LOCALS_QUERY,
        extensions: &["swift"],
        file_names: &[],
        shebangs: &[],
    },
    Parser {
        name: "haskell",
        language: || tree_sitter_haskell::LANGUAGE.into(),
        highlights: &[tree_sitter_haskell::HIGHLIGHTS_QUERY],
        injections: tree_sitter_haskell::INJECTIONS_QUERY,
        locals: tree_sitter_haskell::LOCALS_QUERY,
        extensions: &["hs", "lhs"],
        file_names: &[],
        shebangs: &["runhaskell", "runghc"],
    },
    Parser {
        name: "elixir",
        language: || tree_sitter_elixir::LANGUAGE.into(),
        highlights: &[tree_sitter_elixir::HIGHLIGHTS_QUERY],
        injections: tree_sitter_elixir::INJECTIONS_QUERY,
        locals: "",
        extensions: &["ex", "exs"],
        file_names: &["mix.lock"],
        shebangs: &["elixir"],
    },
    Parser {
        name: "nix",
        language: || tree_sitter_nix::LANGUAGE.into(),
        highlights: &[tree_sitter_nix::HIGHLIGHTS_QUERY],
        injections: tree_sitter_nix::INJECTIONS_QUERY,
        locals: "",
        extensions: &["nix"],
        file_names: &[],
        shebangs: &[],
    },
    Parser {
        name: "sql",
        language: || tree_sitter_sequel::LANGUAGE.into(),
        highlights: &[tree_sitter_sequel::HIGHLIGHTS_QUERY],
        injections: "",
        locals: "",
        extensions: &["sql", "psql", "mysql"],
        file_names: &[],
        shebangs: &[],
    },
];
