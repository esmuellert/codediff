//! A commit hash.

/// A commit hash stored as text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommitHash(String);

impl CommitHash {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CommitHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_hash_preserves_and_displays_its_text() {
        let hash = CommitHash::new("b87b24c36494876cdcf7a866805e50a10774b941");
        assert_eq!(hash.as_str(), "b87b24c36494876cdcf7a866805e50a10774b941");
        assert_eq!(hash.to_string(), "b87b24c36494876cdcf7a866805e50a10774b941");
    }
}
