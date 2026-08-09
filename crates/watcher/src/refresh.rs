//! The vocabulary that crosses the channel: what needs refreshing.

/// A set of things that changed. Multiple events collapse into one by union.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refresh {
    pub worktree: bool,
    pub index: bool,
    pub head: bool,
    pub refs: bool,
}

impl Refresh {
    pub fn is_empty(self) -> bool {
        !self.worktree && !self.index && !self.head && !self.refs
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            worktree: self.worktree || other.worktree,
            index: self.index || other.index,
            head: self.head || other.head,
            refs: self.refs || other.refs,
        }
    }
}

impl std::fmt::Display for Refresh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if self.worktree {
            parts.push("worktree");
        }
        if self.index {
            parts.push("index");
        }
        if self.head {
            parts.push("head");
        }
        if self.refs {
            parts.push("refs");
        }
        if parts.is_empty() {
            write!(f, "(none)")
        } else {
            write!(f, "{}", parts.join("|"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_refresh_displays_as_none() {
        assert_eq!(Refresh::default().to_string(), "(none)");
    }

    #[test]
    fn union_merges_bits() {
        let a = Refresh {
            worktree: true,
            ..Default::default()
        };
        let b = Refresh {
            index: true,
            ..Default::default()
        };
        let c = a.union(b);
        assert!(c.worktree && c.index && !c.head && !c.refs);
    }

    #[test]
    fn is_empty_when_nothing_set() {
        assert!(Refresh::default().is_empty());
        assert!(
            !Refresh {
                head: true,
                ..Default::default()
            }
            .is_empty()
        );
    }
}
