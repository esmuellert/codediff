//! A git object id.

/// A git object id, kept as text.
///
/// Never parsed into bytes: it is only handed back to git or compared, and git
/// prints abbreviated ids of varying length. An id is a content hash, which is
/// what makes [`Rev::Commit`](crate::Rev::Commit) an identity and not a name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Oid(String);

impl Oid {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// True for the all-zero id git prints where an object does not exist,
    /// such as the after side of a deletion.
    pub fn is_null(&self) -> bool {
        !self.0.is_empty() && self.0.chars().all(|c| c == '0')
    }
}

impl std::fmt::Display for Oid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_all_zero_id_means_no_object() {
        assert!(Oid::new("0000000000000000000000000000000000000000").is_null());
        assert!(!Oid::new("b87b24c36494876cdcf7a866805e50a10774b941").is_null());
    }

    #[test]
    fn an_empty_id_is_not_the_null_id() {
        // Git prints the zeroes; an empty string is a parse that went wrong,
        // and calling it "no object" would hide that.
        assert!(!Oid::new("").is_null());
    }
}
