//! Zero-based byte, character, UTF-16, and terminal-cell coordinates.
//!
//! The diff engine's one-based UTF-16 boundary conversion lives on
//! `Utf16Col`.

macro_rules! coordinate {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name(pub u32);

        impl $name {
            pub const ZERO: Self = Self(0);

            /// The underlying number. Verbose on purpose: reaching for this
            /// is how a coordinate ends up used as the wrong kind.
            pub const fn get(self) -> u32 {
                self.0
            }

            pub const fn saturating_add(self, n: u32) -> Self {
                Self(self.0.saturating_add(n))
            }

            pub const fn saturating_sub(self, n: u32) -> Self {
                Self(self.0.saturating_sub(n))
            }
        }

        impl From<$name> for usize {
            fn from(value: $name) -> Self {
                value.0 as usize
            }
        }
    };
}

coordinate! {
    /// A byte offset into a UTF-8 string. What `&str` slicing needs.
    ByteOff
}

coordinate! {
    /// An index into `str::chars`, counting Unicode scalar values.
    CharIdx
}

coordinate! {
    /// An offset in UTF-16 code units.
    ///
    /// This is what the diff engine reports, because it mirrors VSCode and
    /// JavaScript strings are UTF-16. Characters outside the Basic
    /// Multilingual Plane count as two: `🎉` is one char but two UTF-16 units.
    Utf16Col
}

coordinate! {
    /// A terminal column. Wide characters occupy two, combining marks zero.
    CellCol
}

impl Utf16Col {
    /// Converts a one-based column as reported by the diff engine.
    ///
    /// Column 0 is treated as column 1; the engine does not emit it, but
    /// clamping is preferable to underflow.
    pub const fn from_engine(column: u32) -> Self {
        Self(column.saturating_sub(1))
    }

    /// Converts back to the engine's one-based convention.
    pub const fn to_engine(self) -> u32 {
        self.0.saturating_add(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_columns_round_trip() {
        assert_eq!(Utf16Col::from_engine(1), Utf16Col(0));
        assert_eq!(Utf16Col::from_engine(13), Utf16Col(12));
        assert_eq!(Utf16Col::from_engine(13).to_engine(), 13);
    }

    #[test]
    fn engine_column_zero_clamps_instead_of_underflowing() {
        assert_eq!(Utf16Col::from_engine(0), Utf16Col(0));
    }

    #[test]
    fn the_extremes_saturate_rather_than_wrapping() {
        assert_eq!(Utf16Col(u32::MAX).to_engine(), u32::MAX);
        assert_eq!(Utf16Col::from_engine(u32::MAX), Utf16Col(u32::MAX - 1));
    }
}
