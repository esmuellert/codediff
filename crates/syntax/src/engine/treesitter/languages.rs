//! Parser languages and query overrides.
//!
//! Each entry supplies the grammar, queries, injections, locals, and detection
//! clues required by one language crate.

use tree_sitter::Language;

/// One parser entry.
pub struct Parser {
    /// Name used by injection queries.
    pub name: &'static str,
    pub language: fn() -> Language,
    /// Highlight queries, joined in the order required by the grammar.
    pub highlights: &'static [&'static str],
    pub injections: &'static str,
    pub locals: &'static str,
    /// Lowercase extensions without the dot.
    pub extensions: &'static [&'static str],
    /// Complete file names that identify the language.
    pub file_names: &'static [&'static str],
    /// Interpreter names accepted in shebangs.
    pub shebangs: &'static [&'static str],
}

/// Rules appended after a grammar's own query.
///
/// Later patterns win in tree-sitter, so appending is how we override.
/// Each fixes a case the matcher already gets right.
mod overrides {
    /// JSON keys need a more specific capture than strings.
    pub const JSON: &str = "(pair key: (_) @string.special.key)";

    /// Restore the function capture for Go methods.
    pub const GO: &str = "(method_declaration name: (field_identifier) @function.method)";

    /// Capture decorator names rather than the whole decorator expression.
    pub const PYTHON: &str = r#"
        (decorator (identifier) @attribute)
        (decorator (attribute) @attribute)
        ((identifier) @variable.builtin (#any-of? @variable.builtin "self" "cls"))
    "#;
    pub const TYPESCRIPT: &str = r#"
        (decorator (identifier) @attribute)
        (decorator (call_expression function: (identifier) @attribute))
    "#;

    /// Mark regular-expression delimiters as string content.
    pub const JAVASCRIPT: &str = r#"(regex "/" @string.special)"#;

    /// Add the character-literal capture used by C and Rust.
    pub const C: &str = "(char_literal) @character";
    pub const RUST: &str = "(char_literal) @character";
}

/// Every language we parse. The engine stores a parser index into this table.
///
/// Adding one: add the crate dependency and a row here.
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
        // JSX uses the JavaScript grammar with extra query rules.
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
        // Compose TypeScript's query with JavaScript's.
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
        // TSX uses a separate grammar because `<T>` has different meaning.
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
        // Compose C++'s query with C's.
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
        // HTML injections cover embedded JavaScript and CSS.
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
