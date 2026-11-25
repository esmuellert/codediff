//! Parses `git log` commit records.
//!
//! Fields are NUL-terminated. Decorations stay as Git spelled them under
//! `--decorate=full`; the repository layer turns records into [`crate::Commit`].

use crate::Repo;
use crate::error::{Error, Result};
use crate::git::{rev_parse, run};

/// Which commit walk ordering `git log` should use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Topological,
    CommitDate,
    AuthorDate,
}

impl Order {
    fn flag(self) -> &'static str {
        match self {
            Order::Topological => "--topo-order",
            Order::CommitDate => "--date-order",
            Order::AuthorDate => "--author-date-order",
        }
    }
}

/// One raw record of `git log`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub hash: String,
    pub parents: Vec<String>,
    pub decorations: Vec<String>,
    pub subject: String,
    pub author: String,
    pub date: String,
}

const FORMAT: &str = "--format=%h%x00%p%x00%D%x00%s%x00%an%x00%cs%x00";

/// Lists up to `limit` raw commit records in `order`, skipping the first `skip`.
pub fn list(repo: &Repo, order: Order, skip: usize, limit: usize) -> Result<Vec<Entry>> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let skipped = format!("--skip={skip}");
    let count = format!("-n{limit}");
    let mut args = vec![
        "log",
        order.flag(),
        "--decorate=full",
        "--branches",
        "--tags",
        "--remotes",
    ];
    if rev_parse::resolve(repo, "HEAD").is_ok() {
        args.push("HEAD");
    }
    if skip > 0 {
        args.push(&skipped);
    }
    args.push(&count);
    args.push(FORMAT);

    parse(&run::run(&repo.root, &args)?)
}

/// Resolves `revs` to deduplicated commit records in caller order.
pub fn resolve(repo: &Repo, revs: &[String]) -> Result<Vec<Entry>> {
    let mut oids: Vec<String> = Vec::new();
    for rev in revs {
        if let Ok(hash) = rev_parse::resolve(repo, rev)
            && !oids.iter().any(|seen| seen == hash.as_str())
        {
            oids.push(hash.as_str().to_owned());
        }
    }
    if oids.is_empty() {
        return Ok(Vec::new());
    }
    let mut args = vec!["log", "--no-walk=unsorted", "--decorate=full", FORMAT];
    for oid in &oids {
        args.push(oid.as_str());
    }
    args.push("--");
    parse(&run::run(&repo.root, &args)?)
}

/// Parses NUL-delimited `git log` output into raw entries.
pub fn parse(bytes: &[u8]) -> Result<Vec<Entry>> {
    let text = std::str::from_utf8(bytes).map_err(|_| Error::NotUtf8 {
        command: "git log".to_owned(),
    })?;
    if text.is_empty() {
        return Ok(Vec::new());
    }

    let mut fields = text.split('\0');
    let mut entries = Vec::new();
    while let Some(hash) = fields.next() {
        // Git writes a trailing newline after each record's final NUL.
        let hash = hash.trim_start_matches(['\n', '\r']);
        if hash.is_empty() {
            continue;
        }
        let parents = next_field(&mut fields, "commit parents")?
            .split_whitespace()
            .filter(|part| !part.is_empty())
            .map(str::to_owned)
            .collect();
        let decorations = next_field(&mut fields, "commit decorations")?
            .split(", ")
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_owned)
            .collect();
        let subject = next_field(&mut fields, "commit subject")?.to_owned();
        let author = next_field(&mut fields, "commit author")?.to_owned();
        let date = next_field(&mut fields, "commit date")?.to_owned();

        entries.push(Entry {
            hash: hash.to_owned(),
            parents,
            decorations,
            subject,
            author,
            date,
        });
    }
    Ok(entries)
}

fn next_field<'a>(fields: &mut std::str::Split<'a, char>, what: &str) -> Result<&'a str> {
    fields.next().ok_or_else(|| Error::Parse {
        what: format!("missing {what}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(fields: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        for field in fields {
            out.extend_from_slice(field.as_bytes());
            out.push(0);
        }
        out.push(b'\n');
        out
    }

    #[test]
    fn nul_delimited_records_keep_special_characters_and_merge_parents() {
        let mut bytes = record(&[
            "63e1cbe",
            "09d43b9 8339837",
            "HEAD -> refs/heads/main, refs/remotes/origin/main",
            "fix: pipe | comma , quote \" and 'apostrophe'",
            "Author, Jr. <x>",
            "2026-09-09",
        ]);
        bytes.extend_from_slice(&record(&[
            "09d43b9",
            "",
            "tag: refs/tags/v0.1.0",
            "initial",
            "Bot",
            "2026-09-01",
        ]));

        let entries = parse(&bytes).expect("parses");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].hash, "63e1cbe");
        assert_eq!(entries[0].parents, vec!["09d43b9", "8339837"]);
        assert_eq!(
            entries[0].decorations,
            vec!["HEAD -> refs/heads/main", "refs/remotes/origin/main"]
        );
        assert_eq!(
            entries[0].subject,
            "fix: pipe | comma , quote \" and 'apostrophe'"
        );
        assert_eq!(entries[0].author, "Author, Jr. <x>");
        assert_eq!(entries[0].date, "2026-09-09");
        assert!(entries[1].parents.is_empty());
    }

    #[test]
    fn empty_log_output_returns_no_entries() {
        assert!(parse(b"").expect("parses").is_empty());
    }

    #[test]
    fn truncated_record_is_reported_as_parse_error() {
        assert!(parse(b"63e1cbe\0only-parents\0").is_err());
        assert!(parse(b"63e1cbe\0p\0\0subj\0author").is_err());
    }
}
