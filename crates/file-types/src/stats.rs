//! Added and removed line counts.

/// Lines added and removed in one file, or summed over several.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    pub added: u32,
    pub removed: u32,
}

impl Stats {
    pub fn new(added: u32, removed: u32) -> Self {
        Self { added, removed }
    }

    /// Whether both counts are zero.
    pub fn is_empty(self) -> bool {
        self.added == 0 && self.removed == 0
    }
}

impl std::ops::Add for Stats {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            added: self.added + other.added,
            removed: self.removed + other.removed,
        }
    }
}

impl std::iter::Sum for Stats {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), |total, one| total + one)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_add_up_over_a_group_of_files() {
        let total: Stats = [Stats::new(4, 0), Stats::new(2, 3)].into_iter().sum();
        assert_eq!(total, Stats::new(6, 3));
    }

    #[test]
    fn a_file_git_could_not_measure_shows_nothing() {
        assert!(Stats::default().is_empty());
        assert!(!Stats::new(0, 1).is_empty());
    }
}
